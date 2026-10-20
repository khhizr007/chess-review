//! The move coach: explains a move by naming what happens on the board and
//! showing the line that proves it (`docs/coach-analysis.md` §6).
//!
//! The pipeline is pure (no engine calls; it reads the engine lines the review
//! already has, including the null-move threat searches):
//!
//! 1. [`facts`]: attackers, defenders, exchanges, pins, hanging and trapped pieces.
//! 2. [`detect`]: tags such as Fork or HangsPiece for one move in one position.
//! 3. [`circ`]: each tag in its circumstance: the move itself does it (BASE), it
//!    threatens to do it next (THREATENS), it stops the opponent's threat to do
//!    it (PREVENTS), it allows the opponent's reply to do it (PERMITS), the best
//!    move would have done it (MISSES), later in the line (LEADS_TO), winning
//!    material.
//! 4. [`priority`]: one main event, plus, for a bad move, at most one about the
//!    best move (said like a good move: what it wins, does, threatens or
//!    stops), and for a Brilliant sacrifice what it leads to.
//! 5. [`realise`]: sentences from `text/en.toml`, with board marks per fragment
//!    and show-lines narrated move by move.
//!
//! Bad moves (Inaccuracy, Mistake, Blunder, Miss) are explained by what they
//! allow and miss; good moves (Brilliant, Great Find, Best, Excellent, Good) by
//! what they do, threaten and stop. Positional ideas (development, the
//! centre, activity, pawn structure) come after the tactics, and book moves
//! are explained by their development idea alone.
//!
//! Each tag has a minimum rating (`text/en.toml`): above the player's
//! rating it is not said. When a human-move model says how likely the
//! player was to find the best move, or to play the move, the wording says
//! so ("hard to find at this level", "looks natural, but").

pub mod circ;
pub mod detect;
pub mod facts;
pub mod geometry;
pub mod priority;
pub mod realise;
pub mod verify;

use review_core::expected_points::ExpectedOutcome;
use review_core::{Classification, Eval, Phase, Side};
use serde::{Deserialize, Serialize};
use shakmaty::{Chess, Color, Move, Position, Role, Square};
use ts_rs::TS;

use crate::analysis::model::MoveRef;
use crate::board;
use circ::{Circ, Event, Line, LineKind};
use detect::Tag;
use geometry::{Arrow, GeometryView};

/// Who the coach is talking to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Style {
    /// To the player about their own move.
    You,
    /// To the player about the other side's move.
    Opponent,
    /// About the sides by colour.
    #[default]
    Neutral,
}

impl Style {
    /// The style for a move by `mover` when the player is `player` (if known).
    pub fn for_player(player: Option<Side>, mover: Side) -> Style {
        match player {
            Some(p) if p == mover => Style::You,
            Some(_) => Style::Opponent,
            None => Style::Neutral,
        }
    }
}

/// What the coach says about one move.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CoachNote {
    pub sentences: Vec<Sentence>,
    /// Lines to replay on the board, with a narration per move.
    pub show_lines: Vec<ShowLine>,
    /// Every event the coach found for the move (sentences point into this).
    pub events: Vec<CoachEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Sentence {
    pub fragments: Vec<Fragment>,
    /// Index into `events` of the event this sentence says.
    pub event: Option<u32>,
}

/// A piece of a sentence. Fragments that name a move or a piece carry the
/// arrows and squares that show it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Fragment {
    pub text: String,
    pub arrows: Vec<Arrow>,
    pub squares: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ShowLine {
    pub label: String,
    /// Position the line starts from.
    pub start_fen: String,
    pub moves: Vec<MoveRef>,
    /// One sentence per move of `moves`.
    pub narration: Vec<String>,
    pub event: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl From<Role> for PieceKind {
    fn from(role: Role) -> Self {
        match role {
            Role::Pawn => PieceKind::Pawn,
            Role::Knight => PieceKind::Knight,
            Role::Bishop => PieceKind::Bishop,
            Role::Rook => PieceKind::Rook,
            Role::Queen => PieceKind::Queen,
            Role::King => PieceKind::King,
        }
    }
}

/// A board event as sent to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CoachEvent {
    /// Circumstance first, e.g. `[permits, wins_material]`.
    pub chain: Vec<Circ>,
    pub tag: Tag,
    /// The piece in focus and its square.
    pub piece: Option<PieceKind>,
    pub square: Option<String>,
    /// Whose idea it is.
    pub actor: Side,
    pub geometry: GeometryView,
    /// The line that shows it (UCI): the opponent's best reply line for a bad
    /// move's BASE and PERMITS events, the best line for MISSES events, the
    /// played move's own line for a good move's BASE events, and the null-move
    /// line for THREATENS and PREVENTS events (it starts from the position with
    /// the side to move flipped; the event's show-line has its FEN).
    pub line: Vec<String>,
    /// Index in `line` of the move that carries it out (BASE: the reviewed move,
    /// before the line).
    pub ply: u32,
    /// Material the line wins (cp).
    pub gain_cp: Option<i32>,
    pub mate_in: Option<u32>,
}

/// An engine line from a position: its moves and its evaluation.
#[derive(Debug, Clone, Copy)]
pub struct LineInput<'a> {
    pub pv: &'a [String],
    pub eval: Eval,
}

/// One reviewed move with the engine facts around it.
pub struct CoachInput<'a> {
    pub before: &'a Chess,
    pub played: Move,
    /// The opponent's move that led to `before`.
    pub previous: Option<Move>,
    /// R: the best line from the position after the played move.
    pub reply: Option<LineInput<'a>>,
    /// B: the best line from `before`.
    pub best: Option<LineInput<'a>>,
    /// The played move's own line from `before` (the played move first).
    pub played_line: Option<LineInput<'a>>,
    /// N: the null-move threat search after the played move (its mover to move
    /// again; see [`board::null_move`]).
    pub threat: Option<LineInput<'a>>,
    /// N_prev: the null-move threat search before the played move (the
    /// opponent to move).
    pub prev_threat: Option<LineInput<'a>>,
    /// N_B: the null-move threat search after the best move B (its mover to
    /// move again), when B is not the played move.
    pub best_threat: Option<LineInput<'a>>,
    /// B is the only move within the Good band: every other engine line loses
    /// more expected points than a Good move may.
    pub only_move: bool,
    /// The square of the piece the played move gives up, when it is a
    /// sacrifice (static exchange; see `analysis::detectors::static_sacrifice`).
    pub sacrifice: Option<Square>,
    /// The same for the best move B.
    pub best_sacrifice: Option<Square>,
    /// The expected outcome for the mover before and after the move.
    pub swing: Option<(ExpectedOutcome, ExpectedOutcome)>,
    pub classification: Option<Classification>,
    /// The game phase after the move.
    pub phase: Phase,
    /// The player's rating for the rating gates: tags whose `min_rating` is
    /// above it are not said. `None` says every tag.
    pub rating: Option<i32>,
    /// How likely players of the mover's rating are to play the move and the
    /// best move (from a human-move model), when known.
    pub difficulty: Option<Difficulty>,
    pub style: Style,
    /// Plies of a line to inspect for material (extended through exchanges).
    pub horizon: u32,
    /// Picks among the wordings of a sentence (the ply number keeps it stable).
    pub seed: u32,
}

/// The chance that a player of the mover's rating plays a move (a human-move
/// model's prediction; `docs/coach-analysis.md` §6.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Difficulty {
    /// P(played move).
    pub played: f32,
    /// P(best move).
    pub best: f32,
}

/// Below this chance the best move is hard to find at the player's level:
/// the text about it is softened.
pub const HARD_TO_FIND: f32 = 0.10;
/// From this chance a bad move is a natural one at the player's level: the
/// text leads with the safety check it skipped.
pub const NATURAL: f32 = 0.30;

/// Bad moves: explained by what they allow and miss.
pub fn is_bad(classification: Option<Classification>) -> bool {
    matches!(
        classification,
        Some(
            Classification::Inaccuracy
                | Classification::Mistake
                | Classification::Blunder
                | Classification::Miss
        )
    )
}

/// Good moves: explained by what they do, threaten and stop.
pub fn is_good(classification: Option<Classification>) -> bool {
    matches!(
        classification,
        Some(
            Classification::Brilliant
                | Classification::GreatFind
                | Classification::Best
                | Classification::Excellent
                | Classification::Good
        )
    )
}

/// Classifications the coach explains (not Book, Forced or unclassified moves).
pub fn explains(classification: Option<Classification>) -> bool {
    is_bad(classification) || is_good(classification)
}

/// Moves to mate for `solver` when `eval` is its forced mate.
fn mate_for(eval: Eval, solver: Color) -> Option<u32> {
    match eval {
        Eval::Mate { winner, moves } if winner == board::side(solver) && moves > 0 => Some(moves),
        _ => None,
    }
}

/// The tag may be said at the input's rating.
fn allowed(input: &CoachInput<'_>, tag: Tag) -> bool {
    input.rating.is_none_or(|r| realise::min_rating(tag) <= r)
}

/// Coaches one move. A forced move is said to be the only legal move; a book
/// move gets a note when it develops, castles or takes the centre; other
/// moves the coach does not explain get an empty note.
pub fn coach_move(input: &CoachInput<'_>) -> CoachNote {
    let before = input.before;
    let mover = before.turn();
    if input.classification == Some(Classification::Book) {
        return book_note(input);
    }
    if input.classification == Some(Classification::Forced) {
        let voice = realise::Voice {
            style: input.style,
            mover,
        };
        return CoachNote {
            sentences: vec![realise::forced(before, input.played, voice, input.seed)],
            ..CoachNote::default()
        };
    }
    if !explains(input.classification) {
        return CoachNote::default();
    }
    let mut after = before.clone();
    after.play_unchecked(input.played);
    let reply = input
        .reply
        .map(|r| {
            let mate = mate_for(r.eval, mover.other());
            Line::parse(&after, r.pv, mate, input.horizon, Some(input.played))
        })
        .filter(|l| !l.moves.is_empty());
    let best = input
        .best
        .map(|b| {
            Line::parse(
                before,
                b.pv,
                mate_for(b.eval, mover),
                input.horizon,
                input.previous,
            )
        })
        .filter(|l| l.moves.first().is_some_and(|m| *m != input.played));
    let own = input
        .played_line
        .map(|p| {
            Line::parse(
                before,
                p.pv,
                mate_for(p.eval, mover),
                input.horizon,
                input.previous,
            )
        })
        .filter(|l| l.moves.first() == Some(&input.played));
    // Null-move lines start from the position with the side to move flipped.
    let null_line = |pos: &Chess, line: Option<LineInput<'_>>, solver: Color| {
        let line = line?;
        let start = board::null_move(pos)?;
        Some(Line::parse(
            &start,
            line.pv,
            mate_for(line.eval, solver),
            input.horizon,
            None,
        ))
        .filter(|l| !l.moves.is_empty())
    };
    let threat = null_line(&after, input.threat, mover);
    let prev_threat = null_line(before, input.prev_threat, mover.other());
    let best_threat = best
        .as_ref()
        .and_then(|b| null_line(&b.positions[1], input.best_threat, mover));

    let good = is_good(input.classification);
    let brilliant = input.classification == Some(Classification::Brilliant);
    let mut events = Vec::new();
    if good {
        events.extend(own.as_ref().and_then(|o| circ::own_line(o, input.previous)));
        events.extend(circ::good_base_events(
            before,
            input.played,
            input.previous,
            own.as_ref(),
            LineKind::Played,
            Circ::Base,
            prev_threat.as_ref(),
        ));
        events.extend(
            prev_threat
                .as_ref()
                .zip(reply.as_ref())
                .and_then(|(p, r)| circ::prevents(p, r)),
        );
        if brilliant && let Some(sq) = input.sacrifice {
            events.extend(circ::sacrifice(
                before,
                input.played,
                sq,
                Circ::Base,
                LineKind::Played,
            ));
        }
        if input.only_move && own.is_some() {
            events.push(whole_move(
                Tag::OnlyMove,
                Circ::Base,
                mover,
                LineKind::Played,
            ));
        }
        events.extend(circ::positional_events(
            before,
            input.played,
            input.previous,
            own.as_ref(),
            LineKind::Played,
            Circ::Base,
            input.phase,
        ));
    } else {
        events.extend(circ::base_events(
            before,
            input.played,
            &after,
            input.previous,
            reply.as_ref(),
            best.as_ref(),
        ));
        events.extend(reply.as_ref().and_then(|r| circ::permits(r, input.played)));
        events.extend(circ::positional_errors(
            before,
            input.played,
            &after,
            input.previous,
            reply.as_ref(),
            input.phase,
        ));
        events.extend(reply.as_ref().and_then(circ::permits_weakness));
        if let Some(b) = best.as_ref() {
            events.extend(circ::best_events(
                before,
                b,
                input.previous,
                prev_threat.as_ref(),
                best_threat.as_ref(),
                input.horizon,
                input.phase,
            ));
            // Answering a check is the point of the move, not what it leaves.
            if let Some(sq) = input.best_sacrifice.filter(|_| !before.is_check()) {
                events.extend(circ::sacrifice(
                    before,
                    b.moves[0],
                    sq,
                    Circ::Misses,
                    LineKind::Best,
                ));
            }
            if input.only_move {
                events.push(whole_move(
                    Tag::OnlyMove,
                    Circ::Misses,
                    mover,
                    LineKind::Best,
                ));
            }
        }
        let slid = input
            .swing
            .is_some_and(|(from, to)| realise::Outlook::of(to) < realise::Outlook::of(from));
        if input.classification == Some(Classification::Inaccuracy) && slid && reply.is_some() {
            events.push(whole_move(Tag::Slide, Circ::Base, mover, LineKind::Reply));
        }
    }
    events.extend(
        threat
            .as_ref()
            .and_then(|t| circ::threatens(t, before, input.played, reply.as_ref())),
    );

    // Ideas above the player's level are not said.
    events.retain(|e| allowed(input, e.tag()));

    let miss = input.classification == Some(Classification::Miss);
    let selection = if good {
        priority::select_good(&events)
    } else {
        priority::select_bad(&events, miss)
    };
    let ctx = realise::Context {
        voice: realise::Voice {
            style: input.style,
            mover,
        },
        seed: input.seed,
        before,
        played: input.played,
        previous: input.previous,
        reply: reply.as_ref(),
        best: best.as_ref(),
        own: own.as_ref(),
        threat: threat.as_ref(),
        prev_threat: prev_threat.as_ref(),
        best_threat: best_threat.as_ref(),
        swing: input.swing,
        miss,
        good,
        special: match input.classification {
            Some(Classification::Brilliant) => Some(realise::Special::Brilliant),
            Some(Classification::GreatFind) => Some(realise::Special::GreatFind),
            _ => None,
        },
        hard: !good && input.difficulty.is_some_and(|d| d.best < HARD_TO_FIND),
        natural: matches!(
            input.classification,
            Some(Classification::Blunder | Classification::Mistake)
        ) && input.difficulty.is_some_and(|d| d.played >= NATURAL),
    };
    let (sentences, show_lines) = realise::realise(&events, selection, &ctx);
    let line_of = |kind: LineKind| ctx.line(kind);
    CoachNote {
        sentences,
        show_lines,
        events: events.iter().map(|e| view(e, line_of(e.line))).collect(),
    }
}

/// A book move is explained only by its development or centre idea, when it
/// has one: "Nf3 is a book move: it develops ...".
fn book_note(input: &CoachInput<'_>) -> CoachNote {
    let before = input.before;
    let own = input
        .played_line
        .map(|p| Line::parse(before, p.pv, None, input.horizon, input.previous))
        .filter(|l| l.moves.first() == Some(&input.played));
    let events: Vec<Event> = circ::positional_events(
        before,
        input.played,
        input.previous,
        own.as_ref(),
        LineKind::Played,
        Circ::Base,
        input.phase,
    )
    .into_iter()
    .filter(|e| e.tag().is_development() && allowed(input, e.tag()))
    .collect();
    let Some(main) = (0..events.len()).min_by_key(|&i| events[i].tag()) else {
        return CoachNote::default();
    };
    let ctx = realise::Context {
        voice: realise::Voice {
            style: input.style,
            mover: before.turn(),
        },
        seed: input.seed,
        before,
        played: input.played,
        previous: input.previous,
        reply: None,
        best: None,
        own: own.as_ref(),
        threat: None,
        prev_threat: None,
        best_threat: None,
        swing: input.swing,
        miss: false,
        good: true,
        special: Some(realise::Special::Book),
        hard: false,
        natural: false,
    };
    let selection = priority::Selection {
        main: Some(main),
        ..priority::Selection::default()
    };
    let (sentences, show_lines) = realise::realise(&events, selection, &ctx);
    CoachNote {
        sentences,
        show_lines,
        events: events.iter().map(|e| view(e, ctx.line(e.line))).collect(),
    }
}

/// An event about a move as a whole, with no piece in focus: the only good
/// move, or a slide in the evaluation.
fn whole_move(tag: Tag, circ: Circ, mover: Color, line: LineKind) -> Event {
    let found = detect::Found {
        tag,
        focus: None,
        targets: Vec::new(),
        geometry: geometry::Geometry::Squares(Vec::new()),
        stake: 0,
    };
    circ::move_event(vec![circ], found, mover, line)
}

fn view(e: &Event, line: Option<&Line>) -> CoachEvent {
    CoachEvent {
        chain: e.chain.clone(),
        tag: e.tag(),
        piece: e.found.focus.map(|p| p.role.into()),
        square: e.found.focus.map(|p| p.square.to_string()),
        actor: board::side(e.actor),
        geometry: e.found.geometry.view(),
        line: line.map(|l| l.uci.clone()).unwrap_or_default(),
        ply: e.ply as u32,
        gain_cp: e.gain.map(|g| g.cp),
        mate_in: e.mate_in,
    }
}

impl CoachNote {
    /// The sentences as plain text.
    pub fn text(&self) -> Vec<String> {
        self.sentences
            .iter()
            .map(|s| s.fragments.iter().map(|f| f.text.as_str()).collect())
            .collect()
    }

    /// The event the main sentence says.
    pub fn main_event(&self) -> Option<&CoachEvent> {
        self.sentences
            .first()
            .and_then(|s| s.event)
            .and_then(|i| self.events.get(i as usize))
    }
}
