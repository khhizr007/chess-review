//! Circumstances: whose idea an event is and when it happens.
//!
//! The nodes per played move are P (the move itself, with its own line), R
//! (the opponent's best reply: the next position's engine line 1 and its PV),
//! B (the best alternative: this position's engine line 1), N (the null-move
//! threat after P: what the mover would play if the opponent passed) and
//! N_prev (the opponent's null-move threat before P), plus N_B (the null-move
//! threat after B). Detectors run on P for BASE, along R's PV for PERMITS,
//! along N's for THREATENS and along N_prev's for PREVENTS. B is said like a
//! played good move under MISSES: its own line, what it does by itself, N_B
//! for what it threatens and N_prev for what it stops. On a line that wins material, the
//! tactic is looked for at every move of its side up to the ply where the
//! material is won: found on the first move it WINS_MATERIAL, found later the
//! line LEADS_TO it.

use serde::Serialize;
use shakmaty::{Chess, Color, Move, Position, Role, Square};
use ts_rs::TS;

use super::detect::{self, Found, MoveCtx, Tag};
use super::facts::{self, PieceAt};
use super::geometry::Geometry;
use crate::analysis::detectors::{LineMaterial, walk_line};
use crate::analysis::see;
use crate::board;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Circ {
    /// The move itself does it.
    Base,
    /// The move threatens it: the mover would do it next if the opponent passed.
    Threatens,
    /// The move stops the opponent's threat to do it.
    Prevents,
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
    /// The played move's own line, from the position before it (the move first).
    Played,
    /// N's PV: the position after the played move with its mover to move again.
    Threat,
    /// N_prev's PV: the position before the played move with the opponent to move.
    PrevThreat,
    /// N_B's PV: the position after the best move with its mover to move again.
    BestThreat,
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
    // What the first mover took last on the way to the gain, and where: the
    // material settles on that capture, not on the other side's move after it.
    let (ply, role) = (0..=ply)
        .rev()
        .filter(|k| k % 2 == 0)
        .find_map(|k| {
            let m = line.moves[k];
            m.capture().or(m.promotion()).map(|r| (k, Some(r)))
        })
        .unwrap_or((ply - ply % 2, None));
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
    let mut found = detect::bad_tags(&ctx, best.and_then(|b| b.moves.first().copied()));
    if !found.iter().any(|f| f.tag == Tag::BadTrade) {
        found.extend(detect::loses_trade(&ctx, reply.map_or(0, Line::net)));
        found.sort_by_key(|f| f.tag);
    }
    found
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

/// The line's first move gives check without capturing: the other side must
/// answer it, so a tactic on the next move still follows from the first.
fn quiet_check(line: &Line) -> bool {
    line.moves.first().is_some_and(|m| !m.is_capture()) && line.positions[1].is_check()
}

/// The event a line carries for its first mover, under circumstance `circ`
/// (PERMITS for R, MISSES for B): a mate, the highest-priority tactic behind
/// the material it wins, or else a tactic of its first move.
///
/// In the opponent's reply line (PERMITS) a tactic is named only on its first
/// move, or on its second after a quiet check. Further down, the material is
/// said as won through no named tactic: the move "allows … to win material".
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
        // A mate pattern is named only on the mating move itself; a longer
        // mate is a forced mate, whatever the final pattern.
        let found = match mate {
            Some((0, found)) => found,
            _ => {
                let first = line.ctx(0, previous);
                let king = line.start.board().king_of(line.solver().other());
                Found {
                    tag: Tag::ForcedMate,
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
        let named_at = |k: usize| circ != Circ::Permits || k == 0 || (k == 2 && quiet_check(line));
        for k in (0..=gain.ply).step_by(2).filter(|k| named_at(*k)) {
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
///
/// A capture that the reply simply takes back, where the piece that takes
/// back cannot be won in turn, gave the piece away: the reply takes a free
/// piece, although it ends an exchange.
///
/// A free piece is one the reply takes and keeps, and the line wins about
/// that much. When the mover takes back on the square (the piece was only
/// defended too little), or the line goes on to win clearly more than the
/// piece, the reply wins material instead.
pub fn permits(reply: &Line, played: Move) -> Option<Event> {
    let e = line_event(reply, LineKind::Reply, Circ::Permits, Some(played))?;
    if e.mate_in.is_some() {
        return Some(e);
    }
    let e = recaptured_for_free(reply, played, e);
    Some(taken_back_or_more(reply, e))
}

/// A free piece on the reply's first move that is taken back, or that the
/// line outgrows, said as the material the line wins (about the same piece).
fn taken_back_or_more(reply: &Line, mut e: Event) -> Event {
    let Some(gain) = e.gain.filter(|_| e.tag() == Tag::FreePiece && e.ply == 0) else {
        return e;
    };
    let first = reply.moves[0];
    let taken_back = reply
        .moves
        .get(1)
        .is_some_and(|m| m.is_capture() && m.to() == first.to());
    let piece = first.capture().map_or(0, facts::value);
    if taken_back || gain.cp > piece + MIN_GAIN {
        e.chain = vec![Circ::Permits];
        e.found.tag = Tag::MaterialGain;
        e.found.stake = gain.cp;
    }
    e
}

/// The reply takes back a capture with a piece that cannot be won in turn:
/// the piece was given away (see [`permits`]).
fn recaptured_for_free(reply: &Line, played: Move, e: Event) -> Event {
    let takes_back = reply
        .moves
        .first()
        .filter(|r| played.is_capture() && r.is_capture() && r.to() == played.to());
    let free = takes_back
        .filter(|r| see::capture_gain(&reply.positions[1], r.to()) == 0)
        .and_then(|_| detect::best_tactic(&reply.ctx(0, None)))
        .filter(|f| f.tag == Tag::FreePiece && f.tag < e.tag());
    match (free, gain_of(reply)) {
        (Some(found), Some(gain)) => Event {
            chain: vec![Circ::Permits, Circ::WinsMaterial],
            found,
            actor: reply.solver(),
            line: LineKind::Reply,
            ply: 0,
            gain: Some(gain),
            mate_in: None,
        },
        _ => e,
    }
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

/// The played move's own line (for good moves): a mate, the tactic behind the
/// material it wins, or else a tactic of the move itself.
pub fn own_line(line: &Line, previous: Option<Move>) -> Option<Event> {
    line_event(line, LineKind::Played, Circ::Base, previous)
}

/// What a good move does by itself (trades, defence, tempo; see
/// [`detect::good_tags`]), said under `circ`: BASE for the played move on its
/// own line, MISSES for the best move on the best line. `line` is that move's
/// line (the move first); `prev_threat` is N_prev, the opponent's null-move
/// line before it.
pub fn good_base_events(
    before: &Chess,
    m: Move,
    previous: Option<Move>,
    line: Option<&Line>,
    kind: LineKind,
    circ: Circ,
    prev_threat: Option<&Line>,
) -> Vec<Event> {
    let mut after = before.clone();
    after.play_unchecked(m);
    let rest = line.and_then(|l| l.moves.get(1..)).unwrap_or(&[]);
    let ctx = MoveCtx {
        before,
        m,
        after: &after,
        previous,
        rest,
        first: true,
    };
    // Taking back can itself come out ahead (a rook taken back with a knight
    // nets the exchange): the gain settles on the move.
    let settles = line.and_then(gain_of).filter(|g| g.ply == 0);
    detect::good_tags(&ctx, prev_threat.map(|l| &l.moves[..]))
        .into_iter()
        .map(|found| {
            let gain = settles.filter(|_| found.tag == Tag::Recapture);
            Event {
                chain: if gain.is_some() {
                    vec![circ, Circ::WinsMaterial]
                } else {
                    vec![circ]
                },
                found,
                actor: before.turn(),
                line: kind,
                ply: 0,
                gain,
                mate_in: None,
            }
        })
        .collect()
}

/// An event about the move as a whole (no line of its own to show beyond
/// `line`): a sacrifice, the only good move, a slide in the evaluation.
pub fn move_event(chain: Vec<Circ>, found: Found, actor: Color, line: LineKind) -> Event {
    Event {
        chain,
        found,
        actor,
        line,
        ply: 0,
        gain: None,
        mate_in: None,
    }
}

/// A sacrifice of the move `m`: the piece on `sq` (after the move) can be
/// taken, and that is the point. The moved piece itself is a `Sacrifice`;
/// another piece left to be taken is `OffersPiece`.
pub fn sacrifice(before: &Chess, m: Move, sq: Square, circ: Circ, line: LineKind) -> Option<Event> {
    let mut after = before.clone();
    after.play_unchecked(m);
    let piece = PieceAt::on(after.board(), sq).filter(|p| p.color == before.turn())?;
    let moved = sq == m.to() && !m.is_castle();
    let geometry = match facts::least_valuable_attacker(after.board(), sq, piece.color.other()) {
        Some(from) => Geometry::Relation { from, to: sq },
        None => Geometry::Squares(vec![sq]),
    };
    let found = Found {
        tag: if moved {
            Tag::Sacrifice
        } else {
            Tag::OffersPiece
        },
        focus: Some(piece),
        targets: Vec::new(),
        geometry,
        stake: 0,
    };
    Some(move_event(vec![circ], found, before.turn(), line))
}

/// The best alternative B said like a played good move, every event under
/// MISSES: what B wins or mates on its own line (with MISSES_SAME for the
/// piece just left hanging), what it does by itself (trades, defence, tempo),
/// what it threatens (N_B, the null-move line after it) and which threat of
/// the opponent it stops (N_prev).
pub fn best_events(
    before: &Chess,
    best: &Line,
    previous: Option<Move>,
    prev_threat: Option<&Line>,
    best_threat: Option<&Line>,
    horizon: u32,
) -> Vec<Event> {
    let Some(&b) = best.moves.first() else {
        return Vec::new();
    };
    let mut events: Vec<Event> = misses(best, previous).into_iter().collect();
    events.extend(good_base_events(
        before,
        b,
        previous,
        Some(best),
        LineKind::Best,
        Circ::Misses,
        prev_threat,
    ));
    // The opponent's best line after B, for what B's threat and defence leave.
    let after_b = Line::parse(&best.positions[1], &best.uci[1..], None, horizon, Some(b));
    let missed = |mut e: Event| {
        e.chain.insert(0, Circ::Misses);
        e
    };
    events.extend(
        best_threat
            .and_then(|t| threatens(t, before, b, Some(&after_b)))
            .map(|mut e| {
                e.line = LineKind::BestThreat;
                missed(e)
            }),
    );
    if !after_b.moves.is_empty() {
        events.extend(prev_threat.and_then(|p| prevents(p, &after_b)).map(missed));
    }
    events
}

/// Where a line makes its gain: the square of the move at the gain ply.
fn gain_square(line: &Line, gain: &Gain) -> shakmaty::Square {
    line.moves[gain.ply].to()
}

/// A threat is its first move: it mates, or its tactic is that move and the
/// material falls by the threatening side's next move. Ideas that need a
/// quiet move first are plans, not threats.
fn immediate(e: &Event) -> bool {
    e.ply == 0 && e.gain.is_none_or(|g| g.ply <= 2)
}

/// THREATENS: what the mover would play next if the opponent passed (N, the
/// null-move line after the move), when it mates or wins material. Winning a
/// piece the mover could already win before the move is no new threat, and
/// nothing is threatened when the opponent's best reply simply takes the
/// moved piece (a trade, or a piece given up). A threat to take one piece that
/// can simply move to a safe square is no threat either: the move gains a
/// tempo (`Kicks`, `GainsTempo`). Double attacks and other tactics stay
/// threats, since moving one piece does not answer them.
pub fn threatens(
    threat: &Line,
    before: &Chess,
    played: Move,
    reply: Option<&Line>,
) -> Option<Event> {
    let taken = reply
        .and_then(|r| r.moves.first())
        .is_some_and(|m| m.is_capture() && m.to() == played.to());
    if taken {
        return None;
    }
    let e = line_event(threat, LineKind::Threat, Circ::Threatens, None)?;
    if !immediate(&e) {
        return None;
    }
    if e.mate_in.is_none() {
        let gain = e.gain?;
        let sq = gain_square(threat, &gain);
        let same_piece =
            before.board().piece_at(sq) == threat.positions[gain.ply].board().piece_at(sq);
        if same_piece && facts::threat(before.board(), sq, threat.solver()) > 0 {
            return None;
        }
        let single = matches!(e.tag(), Tag::FreePiece | Tag::WinsTrade | Tag::MaterialGain);
        if single && same_piece {
            let mut after = before.clone();
            after.play_unchecked(played);
            if facts::has_safe_square(&after, sq) {
                return None;
            }
        }
    }
    Some(e)
}

/// PREVENTS: the opponent's threat before the move (N_prev, the null-move line
/// before it), a mate or material won, that is gone after the move: the
/// opponent's best reply no longer mates, or no longer wins that much and
/// does not take on the threatened square.
pub fn prevents(prev_threat: &Line, reply: &Line) -> Option<Event> {
    let e = line_event(prev_threat, LineKind::PrevThreat, Circ::Prevents, None)?;
    if !immediate(&e) {
        return None;
    }
    let gone = if e.mate_in.is_some() {
        reply.mate_in.is_none()
    } else {
        let gain = e.gain?;
        let sq = gain_square(prev_threat, &gain);
        reply.net() < gain.cp
            && !reply
                .moves
                .iter()
                .step_by(2)
                .take(2)
                .any(|m| m.to() == sq && m.is_capture())
    };
    gone.then_some(e)
}
