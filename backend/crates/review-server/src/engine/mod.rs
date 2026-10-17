//! Position search. The review pipeline depends only on [`Evaluator`]; the
//! Stockfish implementation lives in [`pool`] / [`uci`].

pub mod pool;
pub mod uci;

use async_trait::async_trait;
use review_core::Eval;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

/// Search budget per position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum SearchLimit {
    Depth { depth: u32 },
    MoveTime { millis: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnalysisRequest {
    pub fen: String,
    pub limit: SearchLimit,
    /// Number of principal variations (≥ 1).
    pub multipv: u8,
    /// Chess960 castling semantics (`UCI_Chess960`).
    pub chess960: bool,
}

/// One principal variation. `eval` is the evaluation of the *searched*
/// position assuming this line is played, from White's point of view.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineLine {
    pub rank: u8,
    pub eval: Eval,
    pub depth: u32,
    /// Moves in UCI notation, starting with the candidate move.
    pub pv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PositionAnalysis {
    pub lines: Vec<EngineLine>,
}

impl PositionAnalysis {
    pub fn best(&self) -> Option<&EngineLine> {
        self.lines.first()
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("no engine is configured (set STOCKFISH_PATH)")]
    NotConfigured,
    #[error("could not start the engine: {0}")]
    Spawn(String),
    #[error("engine protocol error: {0}")]
    Protocol(String),
    #[error("engine did not answer in time")]
    Timeout,
    #[error("engine process exited unexpectedly: {0}")]
    Crashed(String),
    #[error("analysis was cancelled")]
    Cancelled,
}

#[async_trait]
pub trait Evaluator: Send + Sync {
    async fn analyze(
        &self,
        request: &AnalysisRequest,
        cancel: &CancellationToken,
    ) -> Result<PositionAnalysis, EngineError>;

    /// Human-readable engine identity (e.g. "Stockfish 19").
    fn name(&self) -> String;

    /// Searches that may usefully run at the same time.
    fn parallelism(&self) -> usize {
        1
    }
}
