//! The move coach: explains a move by naming what happens on the board and
//! showing the line that proves it (`docs/coach-analysis.md` §6).
//!
//! The pipeline is pure (no engine calls; it reads the engine lines the review
//! already has):
//!
//! 1. [`facts`]: attackers, defenders, exchanges, pins, hanging and trapped pieces.
//! 2. [`detect`]: tags such as Fork or HangsPiece for one move in one position.
//! 3. [`circ`]: each tag in its circumstance: the move itself does it (BASE), it
//!    allows the opponent's reply to do it (PERMITS), the best move would have
//!    done it (MISSES), later in the line (LEADS_TO), winning material.
//! 4. [`priority`]: one main event, plus at most one about the best move.
//! 5. [`realise`]: sentences from `text/en.toml`, with board marks per fragment
//!    and show-lines narrated move by move.
//!
//! Phase 1 explains bad moves (Inaccuracy, Mistake, Blunder, Miss) only.

pub mod circ;
pub mod detect;
pub mod facts;
pub mod geometry;
pub mod priority;
pub mod realise;

use review_core::{Classification, Eval, Side};
use serde::Serialize;
use shakmaty::{Chess, Color, Move, Position, Role};
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

/// What the coach says about one move.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct CoachNote {
    pub sentences: Vec<Sentence>,
    /// Lines to replay on the board, with a narration per move.
    pub show_lines: Vec<ShowLine>,
    /// Every event the coach found for the move (sentences point into this).
    pub events: Vec<CoachEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Sentence {
    pub fragments: Vec<Fragment>,
    /// Index into `events` of the event this sentence says.
    pub event: Option<u32>,
}

/// A piece of a sentence. Fragments that name a move or a piece carry the
/// arrows and squares that show it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Fragment {
    pub text: String,
    pub arrows: Vec<Arrow>,
    pub squares: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
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
    /// The line that shows it (UCI): the opponent's best reply line for BASE
    /// and PERMITS events, the best line for MISSES events.
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
    pub classification: Option<Classification>,
    pub style: Style,
    /// Plies of a line to inspect for material (extended through exchanges).
    pub horizon: u32,
    /// Picks among the wordings of a sentence (the ply number keeps it stable).
    pub seed: u32,
}

/// Classifications the coach explains in phase 1.
pub fn explains(classification: Option<Classification>) -> bool {
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

/// Moves to mate for `solver` when `eval` is its forced mate.
fn mate_for(eval: Eval, solver: Color) -> Option<u32> {
    match eval {
        Eval::Mate { winner, moves } if winner == board::side(solver) && moves > 0 => Some(moves),
        _ => None,
    }
}

/// Coaches one move. Moves the coach does not explain get an empty note.
pub fn coach_move(input: &CoachInput<'_>) -> CoachNote {
    if !explains(input.classification) {
        return CoachNote::default();
    }
    let before = input.before;
    let mover = before.turn();
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

    let mut events = circ::base_events(
        before,
        input.played,
        &after,
        input.previous,
        reply.as_ref(),
        best.as_ref(),
    );
    events.extend(reply.as_ref().and_then(|r| circ::permits(r, input.played)));
    events.extend(best.as_ref().and_then(|b| circ::misses(b, input.previous)));

    let selection =
        priority::select_bad(&events, input.classification == Some(Classification::Miss));
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
        miss: input.classification == Some(Classification::Miss),
    };
    let (sentences, show_lines) = realise::realise(&events, selection, &ctx);
    let line_of = |kind: LineKind| match kind {
        LineKind::Reply => reply.as_ref(),
        LineKind::Best => best.as_ref(),
    };
    CoachNote {
        sentences,
        show_lines,
        events: events.iter().map(|e| view(e, line_of(e.line))).collect(),
    }
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
