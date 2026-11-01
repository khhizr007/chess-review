//! Review result types (serialised to the frontend; TypeScript via ts-rs).

use review_core::summary::TallyEntry;
use review_core::{
    Classification, Eval, MissKind, Phase, Precision, ReviewConfig, Rule, Side, SideAccuracy,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::coach::CoachNote;
use crate::engine::SearchLimit;
use crate::openings::OpeningName;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GameInfo {
    pub index: u32,
    pub headers: Vec<Header>,
    pub white: Option<String>,
    pub black: Option<String>,
    pub white_elo: Option<u32>,
    pub black_elo: Option<u32>,
    pub event: Option<String>,
    pub site: Option<String>,
    pub date: Option<String>,
    pub result: Option<String>,
    pub termination: Option<String>,
    pub start_fen: String,
    pub final_fen: String,
    pub chess960: bool,
    pub ply_count: u32,
    pub opening: Option<OpeningName>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MoveRef {
    pub san: String,
    pub uci: String,
    /// Position after this move.
    pub fen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Line {
    pub rank: u32,
    /// Evaluation of the searched position if this line is played (White's view).
    pub eval: Eval,
    pub depth: u32,
    pub moves: Vec<MoveRef>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MoveFlags {
    pub check: bool,
    pub checkmate: bool,
    pub stalemate: bool,
    pub capture: bool,
    pub castle: bool,
    pub promotion: bool,
    pub en_passant: bool,
}

/// Analysis of one played move.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlyReview {
    /// 1-based ply index of this move.
    pub ply: u32,
    pub move_number: u32,
    pub mover: Side,
    pub san: String,
    pub uci: String,
    pub from: Option<String>,
    pub to: String,
    pub fen_before: String,
    pub fen_after: String,
    pub flags: MoveFlags,
    /// Evaluation before / after the move (White's view).
    pub eval_before: Eval,
    pub eval_after: Eval,
    /// Evaluation after the engine's best move (White's view).
    pub eval_best: Option<Eval>,
    /// Expected points of the mover before the move, after it, and after the best move.
    pub ep_before: f64,
    pub ep_after: f64,
    pub ep_best: f64,
    pub ep_lost: f64,
    pub classification: Option<Classification>,
    pub miss_kind: Option<MissKind>,
    pub precision: Option<Precision>,
    pub rule: Rule,
    pub best_move: Option<MoveRef>,
    pub played_is_best: bool,
    /// Reference predicate: whether to present the best move as the suggestion.
    pub show_best_move: bool,
    /// Engine lines from the position before the move (best first).
    pub lines: Vec<Line>,
    /// CAPS2 move accuracy (0–100, may be negative before aggregation).
    pub accuracy: Option<f64>,
    pub phase: Phase,
    pub is_book: bool,
    pub is_key_moment: bool,
    pub is_critical: bool,
    pub material_lost_cp: Option<i32>,
    /// Factual sentences generated from engine output, for the details drawer.
    pub explanation: Vec<String>,
    /// The coach's explanation: the motif behind the move and the line that
    /// shows it. Empty for moves the coach does not explain.
    pub coach: CoachNote,
    /// Comment from the imported PGN, without its `[%…]` commands.
    pub pgn_comment: Option<String>,
    /// The mover's clock after this move (milliseconds), from the PGN's `[%clk]`.
    pub clock_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SideSummary {
    pub side: Side,
    pub name: Option<String>,
    pub elo: Option<u32>,
    pub accuracy: SideAccuracy,
    pub tally: Vec<TallyEntry>,
    pub inaccuracies: u32,
    pub mistakes: u32,
    pub blunders: u32,
    pub misses: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphPoint {
    /// Ply after which this evaluation holds (0 = start position).
    pub ply: u32,
    pub eval: Eval,
    /// White's expected points at the rating used.
    pub white_expected_points: f64,
    pub classification: Option<Classification>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PhaseStarts {
    /// First ply played in the middlegame / endgame.
    pub middlegame: Option<u32>,
    pub endgame: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Strength {
    Fast,
    Standard,
    Deep,
    Maximum,
}

impl Strength {
    pub fn limit(self) -> SearchLimit {
        let nodes = match self {
            Strength::Fast => 150_000,
            Strength::Standard => 600_000,
            Strength::Deep => 1_600_000,
            Strength::Maximum => 6_250_000,
        };
        SearchLimit::Nodes { nodes }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReviewSettingsUsed {
    pub limit: SearchLimit,
    pub multipv: u32,
    /// The side the player played, when the request named it: the coach then
    /// speaks to the player ("you allow …") instead of naming the colours.
    pub player_side: Option<Side>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReviewResult {
    pub game: GameInfo,
    pub engine: String,
    pub settings: ReviewSettingsUsed,
    /// Rating fed to the expected-points curve.
    pub rating_used: i32,
    pub config: ReviewConfig,
    pub initial_eval: Eval,
    pub plies: Vec<PlyReview>,
    pub white: SideSummary,
    pub black: SideSummary,
    pub graph: Vec<GraphPoint>,
    pub key_moments: Vec<u32>,
    pub critical_moments: Vec<u32>,
    /// Plies with the largest expected-points losses (most severe first).
    pub biggest_swings: Vec<u32>,
    pub phase_starts: PhaseStarts,
    /// Known deviations from the reference implementation that affect this review.
    pub notes: Vec<String>,
}
