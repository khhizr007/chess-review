//! HTTP API (documented in `docs/api.md`).

pub mod error;

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use review_core::ReviewConfig;
use serde::{Deserialize, Serialize};
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Color, Position};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use crate::analysis::ReviewRequest;
use crate::analysis::model::{Line, MoveRef, PlyReview, ReviewResult, Strength};
use crate::board;
use crate::engine::{AnalysisRequest, EngineError, Evaluator, SearchLimit};
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
    /// Depth preset; ignored when `depth` or `movetime_ms` is given.
    #[ts(optional)]
    pub strength: Option<Strength>,
    #[ts(optional)]
    pub depth: Option<u32>,
    #[ts(optional)]
    pub movetime_ms: Option<u32>,
    /// Engine lines per position (1–5, default 3).
    #[ts(optional)]
    pub multipv: Option<u8>,
    /// Override review rules (advanced; defaults are the recovered values).
    #[ts(optional)]
    pub config: Option<ReviewConfig>,
}

fn search_limit(
    depth: Option<u32>,
    movetime_ms: Option<u32>,
    strength: Option<Strength>,
) -> Result<SearchLimit, ApiError> {
    match (depth, movetime_ms) {
        (Some(_), Some(_)) => Err(ApiError::bad_request(
            "invalid_settings",
            "give either depth or movetime_ms, not both",
        )),
        (Some(depth), None) if (1..=40).contains(&depth) => Ok(SearchLimit::Depth { depth }),
        (Some(_), None) => Err(ApiError::bad_request(
            "invalid_settings",
            "depth must be between 1 and 40",
        )),
        (None, Some(millis)) if (50..=60_000).contains(&millis) => {
            Ok(SearchLimit::MoveTime { millis })
        }
        (None, Some(_)) => Err(ApiError::bad_request(
            "invalid_settings",
            "movetime_ms must be between 50 and 60000",
        )),
        (None, None) => Ok(SearchLimit::Depth {
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
    let limit = search_limit(req.depth, req.movetime_ms, req.strength)?;
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
    let limit = search_limit(req.depth, req.movetime_ms, Some(Strength::Standard))?;
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
