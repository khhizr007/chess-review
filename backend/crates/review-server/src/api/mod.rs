//! HTTP API (documented in `docs/api.md`).

pub mod error;

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use review_core::{Eval, ReviewConfig, Side};
use serde::{Deserialize, Serialize};
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Color, Position};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use crate::analysis::ReviewRequest;
use crate::analysis::model::{Line, MoveRef, PlyReview, ReviewResult, Strength};
use crate::analysis::sideline::{
    JudgeError, SidelineJudgement, SidelineMove, SidelineThreats, ThreatLine, judge_sideline,
};
use crate::board;
use crate::engine::{AnalysisRequest, EngineError, EngineLine, Evaluator, SearchLimit};
use crate::jobs::{JobStore, ReviewStatus};
use crate::openings::OpeningBook;
use crate::pgn::{ParsedGame, PgnError, parse_pgn};
use error::ApiError;

const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub evaluator: Option<Arc<dyn Evaluator>>,
    /// Why the engine is unavailable, when it is.
    pub engine_error: Option<String>,
    pub openings: Arc<OpeningBook>,
    pub jobs: JobStore,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/config/defaults", get(defaults))
        .route("/api/games/parse", post(parse_games))
        .route("/api/reviews", post(create_review))
        .route(
            "/api/reviews/{id}",
            get(review_status).delete(cancel_review),
        )
        .route("/api/reviews/{id}/result", get(review_result))
        .route("/api/reviews/{id}/plies/{ply}", get(review_ply))
        .route("/api/reviews/{id}/judge", post(judge_sideline_move))
        .route("/api/positions/analyze", post(analyze_position))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

// ---------------------------------------------------------------- health

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Health {
    pub status: String,
    pub version: String,
    pub engine: Option<String>,
    pub engine_error: Option<String>,
    pub opening_positions: u32,
}

async fn health(State(state): State<AppState>) -> Json<Health> {
    Json(Health {
        status: if state.evaluator.is_some() {
            "ok"
        } else {
            "degraded"
        }
        .to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        engine: state.evaluator.as_ref().map(|e| e.name()),
        engine_error: state.engine_error.clone(),
        opening_positions: state.openings.len() as u32,
    })
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Defaults {
    pub config: ReviewConfig,
    pub strengths: Vec<StrengthPreset>,
    pub default_strength: Strength,
    pub default_multipv: u32,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct StrengthPreset {
    pub strength: Strength,
    pub depth: u32,
}

async fn defaults() -> Json<Defaults> {
    Json(Defaults {
        config: ReviewConfig::default(),
        strengths: [
            Strength::Fast,
            Strength::Standard,
            Strength::Deep,
            Strength::Maximum,
        ]
        .into_iter()
        .map(|strength| StrengthPreset {
            strength,
            depth: strength.depth(),
        })
        .collect(),
        default_strength: Strength::Standard,
        default_multipv: 3,
    })
}

// ---------------------------------------------------------------- PGN import

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct ParseRequest {
    pub pgn: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct GameSummary {
    pub index: u32,
    pub white: Option<String>,
    pub black: Option<String>,
    pub white_elo: Option<u32>,
    pub black_elo: Option<u32>,
    pub event: Option<String>,
    pub date: Option<String>,
    pub result: Option<String>,
    pub ply_count: u32,
    pub start_fen: String,
    pub moves: Vec<MoveRef>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ParseEntry {
    pub index: u32,
    pub game: Option<GameSummary>,
    pub error: Option<PgnError>,
    pub error_message: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ParseResponse {
    pub games: Vec<ParseEntry>,
}

fn summarize(game: &ParsedGame) -> GameSummary {
    let header = |n: &str| game.header(n).map(str::to_owned);
    GameSummary {
        index: game.index as u32,
        white: header("White"),
        black: header("Black"),
        white_elo: game.elo(Color::White),
        black_elo: game.elo(Color::Black),
        event: header("Event"),
        date: header("Date").or_else(|| header("UTCDate")),
        result: header("Result").or_else(|| game.movetext_result.clone()),
        ply_count: game.moves.len() as u32,
        start_fen: game.start_fen(),
        moves: game
            .moves
            .iter()
            .zip(&game.positions[1..])
            .map(|(m, pos)| MoveRef {
                san: m.san.clone(),
                uci: m.uci.clone(),
                fen: board::fen(pos),
            })
            .collect(),
    }
}

fn pgn_error(e: PgnError) -> ApiError {
    let code = match e {
        PgnError::Empty => "pgn_empty",
        _ => "pgn_invalid",
    };
    ApiError::bad_request(code, e.to_string()).with_details(e)
}

async fn parse_games(
    body: Result<Json<ParseRequest>, JsonRejection>,
) -> Result<Json<ParseResponse>, ApiError> {
    let Json(req) = body?;
    let games = parse_pgn(&req.pgn).map_err(pgn_error)?;
    Ok(Json(ParseResponse {
        games: games
            .into_iter()
            .enumerate()
            .map(|(i, g)| match g {
                Ok(game) => ParseEntry {
                    index: i as u32,
                    game: Some(summarize(&game)),
                    error: None,
                    error_message: None,
                },
                Err(e) => ParseEntry {
                    index: i as u32,
                    game: None,
                    error_message: Some(e.to_string()),
                    error: Some(e),
                },
            })
            .collect(),
    }))
}

// ---------------------------------------------------------------- reviews

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct CreateReviewRequest {
    pub pgn: String,
    /// Which game of a multi-game PGN (default 0).
    #[ts(optional)]
    pub game_index: Option<u32>,
    /// Depth preset; ignored when `depth`, `movetime_ms` or `nodes` is given.
    #[ts(optional)]
    pub strength: Option<Strength>,
    #[ts(optional)]
    pub depth: Option<u32>,
    #[ts(optional)]
    pub movetime_ms: Option<u32>,
    /// Node budget per position (UCI `go nodes`).
    #[ts(optional)]
    pub nodes: Option<u32>,
    /// Engine lines per position (1–5, default 3).
    #[ts(optional)]
    pub multipv: Option<u8>,
    /// Override review rules (advanced; defaults are the recovered values).
    #[ts(optional)]
    pub config: Option<ReviewConfig>,
    /// The side the user played. The coach then talks to them ("you allow …",
    /// "your opponent missed …"); without it, it names the colours.
    #[ts(optional)]
    pub player_side: Option<Side>,
}

fn search_limit(
    depth: Option<u32>,
    movetime_ms: Option<u32>,
    nodes: Option<u32>,
    strength: Option<Strength>,
) -> Result<SearchLimit, ApiError> {
    match (depth, movetime_ms, nodes) {
        (None, None, Some(nodes)) if (1_000..=50_000_000).contains(&nodes) => {
            Ok(SearchLimit::Nodes { nodes })
        }
        (None, None, Some(_)) => Err(ApiError::bad_request(
            "invalid_settings",
            "nodes must be between 1000 and 50000000",
        )),
        (_, _, Some(_)) | (Some(_), Some(_), None) => Err(ApiError::bad_request(
            "invalid_settings",
            "give only one of depth, movetime_ms and nodes",
        )),
        (Some(depth), None, None) if (1..=40).contains(&depth) => Ok(SearchLimit::Depth { depth }),
        (Some(_), None, None) => Err(ApiError::bad_request(
            "invalid_settings",
            "depth must be between 1 and 40",
        )),
        (None, Some(millis), None) if (50..=60_000).contains(&millis) => {
            Ok(SearchLimit::MoveTime { millis })
        }
        (None, Some(_), None) => Err(ApiError::bad_request(
            "invalid_settings",
            "movetime_ms must be between 50 and 60000",
        )),
        (None, None, None) => Ok(SearchLimit::Depth {
            depth: strength.unwrap_or(Strength::Standard).depth(),
        }),
    }
}

fn require_engine(state: &AppState) -> Result<Arc<dyn Evaluator>, ApiError> {
    state.evaluator.clone().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "engine_unavailable",
            state
                .engine_error
                .clone()
                .unwrap_or_else(|| EngineError::NotConfigured.to_string()),
        )
    })
}

async fn create_review(
    State(state): State<AppState>,
    body: Result<Json<CreateReviewRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<ReviewStatus>), ApiError> {
    let Json(req) = body?;
    let evaluator = require_engine(&state)?;
    let limit = search_limit(req.depth, req.movetime_ms, req.nodes, req.strength)?;
    let multipv = req.multipv.unwrap_or(3);
    if !(1..=5).contains(&multipv) {
        return Err(ApiError::bad_request(
            "invalid_settings",
            "multipv must be between 1 and 5",
        ));
    }
    let index = req.game_index.unwrap_or(0) as usize;
    let mut games = parse_pgn(&req.pgn).map_err(pgn_error)?;
    if index >= games.len() {
        return Err(ApiError::bad_request(
            "game_not_found",
            format!(
                "game_index {index} is out of range: the PGN contains {} game(s)",
                games.len()
            ),
        ));
    }
    let game = games.swap_remove(index).map_err(pgn_error)?;
    if game.moves.is_empty() {
        return Err(ApiError::bad_request(
            "no_moves",
            "the selected game has no moves to review",
        ));
    }
    let request = ReviewRequest {
        game,
        limit,
        multipv,
        config: req.config.unwrap_or_default(),
        player_side: req.player_side,
    };
    let status = state
        .jobs
        .submit(request, evaluator, Arc::clone(&state.openings));
    Ok((StatusCode::ACCEPTED, Json(status)))
}

fn parse_id(id: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(id).map_err(|_| ApiError::not_found("review"))
}

async fn review_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ReviewStatus>, ApiError> {
    state
        .jobs
        .status(parse_id(&id)?)
        .map(Json)
        .ok_or_else(|| ApiError::not_found("review"))
}

async fn cancel_review(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ReviewStatus>, ApiError> {
    state
        .jobs
        .cancel(parse_id(&id)?)
        .map(Json)
        .ok_or_else(|| ApiError::not_found("review"))
}

fn completed(state: &AppState, id: &str) -> Result<Arc<ReviewResult>, ApiError> {
    let (status, result) = state
        .jobs
        .result(parse_id(id)?)
        .ok_or_else(|| ApiError::not_found("review"))?;
    result.ok_or_else(|| {
        ApiError::new(
            StatusCode::CONFLICT,
            "review_not_ready",
            format!("the review is {:?}", status.state),
        )
        .with_details(status)
    })
}

async fn review_result(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ReviewResult>, ApiError> {
    Ok(Json(completed(&state, &id)?.as_ref().clone()))
}

async fn review_ply(
    State(state): State<AppState>,
    Path((id, ply)): Path<(String, u32)>,
) -> Result<Json<PlyReview>, ApiError> {
    let review = completed(&state, &id)?;
    review
        .plies
        .iter()
        .find(|p| p.ply == ply)
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("ply"))
}

// ---------------------------------------------------------------- sideline moves

/// An engine line as the browser engine reports it (docs/explore-mode.md §5.5).
#[derive(Deserialize, TS)]
#[ts(export)]
pub struct JudgeLine {
    /// Evaluation of the position if the line is played (White's view).
    pub eval: Eval,
    pub depth: u32,
    /// Moves in UCI notation, starting with the candidate move.
    pub pv: Vec<String>,
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct JudgeMoveRequest {
    /// Game ply the sideline branches from.
    pub from_ply: u32,
    /// The sideline's moves (UCI) up to and including the move judged.
    pub moves: Vec<String>,
    /// Engine lines of the position before the judged move, best first.
    pub before: Vec<JudgeLine>,
    /// Best line of the position after it, unless the move is one of `before`'s lines.
    #[ts(optional)]
    pub after: Option<JudgeLine>,
    /// Expected points lost by the previous sideline move (its judgement's `ep_lost`).
    #[ts(optional)]
    pub previous_ep_lost: Option<f64>,
    /// The browser engine's null-move threat searches, for the coach's note.
    #[ts(optional)]
    pub threats: Option<JudgeThreats>,
}

/// A null-move threat search: the position searched and its best line.
#[derive(Deserialize, TS)]
#[ts(export)]
pub struct JudgeThreat {
    pub fen: String,
    pub line: JudgeLine,
}

/// The coach's threat searches around the judged move (`docs/explore-mode.md` §5.5).
#[derive(Deserialize, TS)]
#[ts(export)]
pub struct JudgeThreats {
    /// After the move, its mover to move again.
    #[ts(optional)]
    pub after: Option<JudgeThreat>,
    /// Before the move, the opponent to move.
    #[ts(optional)]
    pub before: Option<JudgeThreat>,
    /// After the engine's best move (`before`'s first line), its mover to move again.
    #[ts(optional)]
    pub best: Option<JudgeThreat>,
}

fn engine_line(rank: usize, line: JudgeLine) -> EngineLine {
    EngineLine {
        rank: u8::try_from(rank).unwrap_or(u8::MAX),
        eval: line.eval,
        depth: line.depth,
        pv: line.pv,
    }
}

/// Classifies a move the reader played in a sideline of a finished review, with the review's
/// rules and the browser engine's lines. No engine search is made.
async fn judge_sideline_move(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Result<Json<JudgeMoveRequest>, JsonRejection>,
) -> Result<Json<SidelineJudgement>, ApiError> {
    let Json(req) = body?;
    let review = completed(&state, &id)?;
    let before = req
        .before
        .into_iter()
        .enumerate()
        .map(|(i, l)| engine_line(i + 1, l))
        .collect();
    let after = req.after.map(|l| engine_line(1, l));
    let threat = |t: Option<JudgeThreat>| {
        t.map(|t| ThreatLine {
            fen: t.fen,
            line: engine_line(1, t.line),
        })
    };
    let threats = req
        .threats
        .map_or_else(SidelineThreats::default, |t| SidelineThreats {
            after: threat(t.after),
            before: threat(t.before),
            best: threat(t.best),
        });
    let request = SidelineMove {
        from_ply: req.from_ply,
        moves: &req.moves,
        before,
        after,
        previous_ep_lost: req.previous_ep_lost,
        threats,
    };
    judge_sideline(&review, &state.openings, &request)
        .map(Json)
        .map_err(|e| match e {
            JudgeError::Replay => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "replay_failed",
                e.to_string(),
            ),
            JudgeError::BadPly(_) => ApiError::bad_request("invalid_ply", e.to_string()),
            JudgeError::BadLength => ApiError::bad_request("invalid_moves", e.to_string()),
            JudgeError::IllegalMove(_) => ApiError::bad_request("illegal_move", e.to_string()),
            JudgeError::BadLines => ApiError::bad_request("invalid_lines", e.to_string()),
            JudgeError::MissingAfter => ApiError::bad_request("missing_after", e.to_string()),
        })
}

// ---------------------------------------------------------------- positions

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct AnalyzePositionRequest {
    pub fen: String,
    #[ts(optional)]
    pub depth: Option<u32>,
    #[ts(optional)]
    pub movetime_ms: Option<u32>,
    #[ts(optional)]
    pub multipv: Option<u8>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct PositionAnalysisResponse {
    pub fen: String,
    pub lines: Vec<Line>,
    /// Set when the position is already finished (mate or stalemate).
    pub terminal: Option<String>,
}

async fn analyze_position(
    State(state): State<AppState>,
    body: Result<Json<AnalyzePositionRequest>, JsonRejection>,
) -> Result<Json<PositionAnalysisResponse>, ApiError> {
    let Json(req) = body?;
    let pos: Chess = req
        .fen
        .parse::<Fen>()
        .map_err(|e| e.to_string())
        .and_then(|f| {
            f.into_position(CastlingMode::Standard)
                .map_err(|e| e.to_string())
        })
        .map_err(|reason| ApiError::bad_request("invalid_fen", format!("invalid FEN: {reason}")))?;
    let fen = board::fen(&pos);
    if pos.is_checkmate() || pos.is_stalemate() {
        let terminal = if pos.is_checkmate() {
            "checkmate"
        } else {
            "stalemate"
        };
        return Ok(Json(PositionAnalysisResponse {
            fen,
            lines: Vec::new(),
            terminal: Some(terminal.to_owned()),
        }));
    }
    let evaluator = require_engine(&state)?;
    let limit = search_limit(req.depth, req.movetime_ms, None, Some(Strength::Standard))?;
    let multipv = req.multipv.unwrap_or(3).clamp(1, 5);
    let analysis = evaluator
        .analyze(
            &AnalysisRequest {
                fen: fen.clone(),
                limit,
                multipv,
                chess960: false,
                searchmoves: None,
            },
            &CancellationToken::new(),
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "engine_error", e.to_string()))?;
    let lines = analysis
        .lines
        .iter()
        .map(|l| Line {
            rank: u32::from(l.rank),
            eval: l.eval,
            depth: l.depth,
            moves: crate::analysis::to_san_line(&pos, &l.pv, 12),
        })
        .collect();
    Ok(Json(PositionAnalysisResponse {
        fen,
        lines,
        terminal: None,
    }))
}
