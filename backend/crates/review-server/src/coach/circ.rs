//! Circumstances: whose idea an event is and when it happens.
//!
//! The nodes per played move are P (the move itself), R (the opponent's best
//! reply: the next position's engine line 1 and its PV) and B (the best
//! alternative: this position's engine line 1). Detectors run on P for BASE,
//! along R's PV for PERMITS and along B's PV for MISSES. On a line that wins
//! material, the tactic is looked for at every move of its side up to the
//! ply where the material is won: found on the first move it WINS_MATERIAL,
//! found later the line LEADS_TO it.

use serde::Serialize;
use shakmaty::{Chess, Color, Move, Position, Role};
use ts_rs::TS;

use super::detect::{self, Found, MoveCtx, Tag};
use super::facts::PieceAt;
use super::geometry::Geometry;
use crate::analysis::detectors::{LineMaterial, walk_line};
use crate::board;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Circ {
    /// The move itself does it.
    Base,
    /// The move allows the opponent's best reply to do it.
    Permits,
    /// The best move would have done it.
    Misses,
    /// The best move would have taken the piece the opponent just left hanging.
    MissesSame,
    /// It happens later in the line.
    LeadsTo,
    /// It wins material.
    WinsMaterial,
}

/// Which line an event is shown on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// R's PV, from the position after the played move.
    Reply,
    /// B's PV, from the position before it.
    Best,
}

/// Smallest material gain (cp) that counts as winning material.
pub const MIN_GAIN: i32 = 100;
/// Longest stretch of a PV the coach looks at.
const MAX_LINE_PLIES: usize = 12;

/// An engine line played out on the board.
pub struct Line {
    pub start: Chess,
    pub moves: Vec<Move>,
    pub uci: Vec<String>,
    /// `positions[k]` is the position before `moves[k]`; one more at the end.
    pub positions: Vec<Chess>,
    /// The line's first mover mates in this many moves.
    pub mate_in: Option<u32>,
    /// Material along the line for its first mover.
    pub walk: LineMaterial,
    /// Material the other side took with the move that led to `start` (cp).
    /// Taking it back is not a gain, so gains are counted from before it.
    pub lead: i32,
}

impl Line {
    /// Plays `pv` from `start`. `lead` is the move that led to `start`, when
    /// the line answers it.
    pub fn parse(
        start: &Chess,
        pv: &[String],
        mate_in: Option<u32>,
        horizon: u32,
        lead: Option<Move>,
    ) -> Line {
        let mut pos = start.clone();
        let mut moves = Vec::new();
        let mut uci = Vec::new();
        let mut positions = vec![pos.clone()];
        for text in pv.iter().take(MAX_LINE_PLIES) {
            let Some(m) = board::parse_uci(&pos, text) else {
                break;
            };
            pos.play_unchecked(m);
            moves.push(m);
            uci.push(text.clone());
            positions.push(pos.clone());
        }
        let walk = walk_line(start, start.turn(), &uci, horizon);
        Line {
            start: start.clone(),
            moves,
            uci,
            positions,
            mate_in,
            walk,
            lead: lead.and_then(|m| m.capture()).map_or(0, board::role_value),
        }
    }

    /// Material the line wins for its first mover, counted from before the
    /// move that led to it.
    pub fn net(&self) -> i32 {
        self.walk.net() - self.lead
    }

    pub fn solver(&self) -> Color {
        self.start.turn()
    }

    /// The move at `ply` in its context.
    pub fn ctx(&self, ply: usize, previous: Option<Move>) -> MoveCtx<'_> {
        MoveCtx {
            before: &self.positions[ply],
            m: self.moves[ply],
            after: &self.positions[ply + 1],
            previous: if ply == 0 {
                previous
            } else {
                Some(self.moves[ply - 1])
            },
            rest: &self.moves[ply + 1..],
            first: ply == 0,
        }
    }
}

/// Material won along a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gain {
    pub cp: i32,
    /// The piece captured where the gain is made, when there is one.
    pub role: Option<Role>,
    /// The ply where it is won.
    pub ply: usize,
}

/// A tag in its circumstance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub chain: Vec<Circ>,
    pub found: Found,
    /// Whose idea it is.
    pub actor: Color,
    pub line: LineKind,
    /// Index in the line of the move that carries it out (0 for BASE events,
    /// whose move is the played move itself).
    pub ply: usize,
    pub gain: Option<Gain>,
    pub mate_in: Option<u32>,
}

impl Event {
    pub fn first(&self) -> Circ {
        self.chain[0]
    }

    pub fn tag(&self) -> Tag {
        self.found.tag
    }
}

fn gain_of(line: &Line) -> Option<Gain> {
    let ply = line.walk.gain_ply? as usize;
    if line.net() < MIN_GAIN || ply >= line.moves.len() {
        return None;
    }
    // What the first mover took last on the way to the gain.
    let role = (0..=ply)
        .rev()
        .filter(|k| k % 2 == 0)
        .find_map(|k| line.moves[k].capture().or(line.moves[k].promotion()));
    Some(Gain {
        cp: line.net(),
        role,
        ply,
    })
}

/// BASE events of the played move: what it does wrong by itself.
pub fn base_events(
    before: &Chess,
    played: Move,
    after: &Chess,
    previous: Option<Move>,
    reply: Option<&Line>,
    best: Option<&Line>,
) -> Vec<Event> {
    let rest = reply.map_or(&[][..], |l| &l.moves[..]);
    let ctx = MoveCtx {
        before,
        m: played,
        after,
        previous,
        rest,
        first: true,
    };
    let gain = reply.and_then(gain_of);
    detect::bad_tags(&ctx, best.and_then(|b| b.moves.first().copied()))
        .into_iter()
        .map(|found| Event {
            chain: vec![Circ::Base],
            found,
            actor: before.turn(),
            line: LineKind::Reply,
            ply: 0,
            gain,
            mate_in: None,
        })
        .collect()
}

/// The event a line carries for its first mover, under circumstance `circ`
/// (PERMITS for R, MISSES for B): a mate, the highest-priority tactic behind
/// the material it wins, or else a tactic of its first move.
pub fn line_event(
    line: &Line,
    kind: LineKind,
    circ: Circ,
    previous: Option<Move>,
) -> Option<Event> {
    if line.moves.is_empty() {
        return None;
    }
    let event = |chain: Vec<Circ>, found: Found, ply: usize, gain: Option<Gain>| Event {
        chain,
        found,
        actor: line.solver(),
        line: kind,
        ply,
        gain,
        mate_in: None,
    };

    if let Some(n) = line.mate_in.filter(|n| *n > 0) {
        let mate = (0..line.moves.len())
            .step_by(2)
            .find(|k| line.positions[k + 1].is_checkmate())
            .and_then(|k| detect::mate(&line.ctx(k, previous)).map(|f| (k, f)));
        let found = match mate {
            Some((0, found)) => found,
            other => {
                let first = line.ctx(0, previous);
                let king = line.start.board().king_of(line.solver().other());
                Found {
                    tag: match other {
                        Some((_, f)) if f.tag == Tag::BackRankMate => Tag::BackRankMate,
                        _ => Tag::ForcedMate,
                    },
                    focus: first.moved(),
                    targets: king
                        .and_then(|k| PieceAt::on(line.start.board(), k))
                        .into_iter()
                        .collect(),
                    geometry: Geometry::Squares(king.into_iter().collect()),
                    stake: 0,
                }
            }
        };
        let mut e = event(vec![circ], found, 0, None);
        e.mate_in = Some(n);
        return Some(e);
    }

    if let Some(gain) = gain_of(line) {
        let mut chosen: Option<(usize, Found)> = None;
        for k in (0..=gain.ply).step_by(2) {
            if let Some(f) = detect::best_tactic(&line.ctx(k, previous))
                && chosen.as_ref().is_none_or(|(_, c)| f.tag < c.tag)
            {
                chosen = Some((k, f));
            }
        }
        return Some(match chosen {
            Some((0, found)) => event(vec![circ, Circ::WinsMaterial], found, 0, Some(gain)),
            Some((k, found)) => event(vec![circ, Circ::LeadsTo], found, k, Some(gain)),
            None => {
                let m = line.moves[gain.ply];
                let found = Found {
                    tag: Tag::MaterialGain,
                    focus: m.capture().map(|role| PieceAt {
                        role,
                        color: line.solver().other(),
                        square: m.to(),
                    }),
                    targets: Vec::new(),
                    geometry: Geometry::Relation {
                        from: m.from().unwrap_or(m.to()),
                        to: m.to(),
                    },
                    stake: gain.cp,
                };
                let chain = if gain.ply == 0 {
                    vec![circ]
                } else {
                    vec![circ, Circ::LeadsTo]
                };
                event(chain, found, gain.ply, Some(gain))
            }
        });
    }

    detect::best_tactic(&line.ctx(0, previous))
        .filter(|f| !f.tag.is_mate())
        .map(|found| event(vec![circ], found, 0, None))
}

/// The opponent's reply event (PERMITS).
pub fn permits(reply: &Line, played: Move) -> Option<Event> {
    line_event(reply, LineKind::Reply, Circ::Permits, Some(played))
}

/// The best move's event (MISSES). Taking a free piece on the square the
/// opponent's last move went to is MISSES_SAME.
pub fn misses(best: &Line, previous: Option<Move>) -> Option<Event> {
    let mut e = line_event(best, LineKind::Best, Circ::Misses, previous)?;
    let same_square =
        previous.is_some_and(|p| best.moves.first().is_some_and(|b| b.to() == p.to()));
    if e.tag() == Tag::FreePiece && e.ply == 0 && same_square {
        e.chain = vec![Circ::MissesSame];
    }
    Some(e)
}
