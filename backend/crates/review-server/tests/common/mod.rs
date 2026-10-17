//! Shared test helpers: fixture loading and a deterministic fake engine.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use review_core::{Eval, Side};
use review_server::board;
use review_server::engine::{
    AnalysisRequest, EngineError, EngineLine, Evaluator, PositionAnalysis,
};
use review_server::openings::OpeningBook;
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Color, Position};
use tokio_util::sync::CancellationToken;

pub fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn openings() -> Arc<OpeningBook> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/openings");
    Arc::new(OpeningBook::load_dir(&dir).expect("opening data").0)
}

/// One-ply engine: scores every legal move by checkmate, stalemate, or material.
/// Deterministic, so reviews built on it can be asserted exactly.
#[derive(Default)]
pub struct FakeEngine {
    pub calls: AtomicU32,
    /// Artificial delay per position (to test progress/cancellation).
    pub delay: Option<Duration>,
}

#[async_trait]
impl Evaluator for FakeEngine {
    async fn analyze(
        &self,
        request: &AnalysisRequest,
        cancel: &CancellationToken,
    ) -> Result<PositionAnalysis, EngineError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(delay) = self.delay {
            tokio::select! {
                () = cancel.cancelled() => return Err(EngineError::Cancelled),
                () = tokio::time::sleep(delay) => {}
            }
        }
        let mode = if request.chess960 {
            CastlingMode::Chess960
        } else {
            CastlingMode::Standard
        };
        let pos: Chess = request
            .fen
            .parse::<Fen>()
            .map_err(|e| EngineError::Protocol(e.to_string()))?
            .into_position(mode)
            .map_err(|e| EngineError::Protocol(e.to_string()))?;
        let mover = pos.turn();
        let allowed = |m: &shakmaty::Move| {
            request
                .searchmoves
                .as_ref()
                .is_none_or(|moves| moves.contains(&board::uci(*m, mode)))
        };
        let mut scored: Vec<(i64, EngineLine)> = pos
            .legal_moves()
            .into_iter()
            .filter(allowed)
            .map(|m| {
                let mut after = pos.clone();
                after.play_unchecked(m);
                let (key, eval) = if after.is_checkmate() {
                    (
                        1_000_000,
                        Eval::Mate {
                            winner: board::side(mover),
                            moves: 1,
                        },
                    )
                } else if after.is_stalemate() {
                    (0, Eval::DRAW)
                } else {
                    let cp = board::balance(after.board(), Color::White);
                    let own = if mover == Color::White { cp } else { -cp };
                    (i64::from(own), Eval::Cp { cp })
                };
                let uci = board::uci(m, mode);
                (
                    key,
                    EngineLine {
                        rank: 0,
                        eval,
                        depth: 1,
                        pv: vec![uci],
                    },
                )
            })
            .collect();
        // Best for the side to move first; ties broken by move text for determinism.
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.pv[0].cmp(&b.1.pv[0])));
        let lines = scored
            .into_iter()
            .take(usize::from(request.multipv.max(1)))
            .enumerate()
            .map(|(i, (_, mut line))| {
                line.rank = i as u8 + 1;
                line
            })
            .collect();
        Ok(PositionAnalysis { lines })
    }

    fn name(&self) -> String {
        "FakeEngine".to_owned()
    }

    fn parallelism(&self) -> usize {
        2
    }
}

/// [`FakeEngine`] with fixed lines for chosen positions: `(white cp, pv)` per
/// line, best first, keyed by the first four FEN fields.
#[derive(Default)]
pub struct ScriptedEngine {
    pub fallback: FakeEngine,
    pub scripts: HashMap<String, Vec<(i32, Vec<String>)>>,
}

impl ScriptedEngine {
    pub fn script(mut self, fen: &str, lines: &[(i32, &[&str])]) -> Self {
        let lines = lines
            .iter()
            .map(|(cp, pv)| (*cp, pv.iter().map(|m| (*m).to_owned()).collect()))
            .collect();
        self.scripts.insert(position_key(fen), lines);
        self
    }
}

fn position_key(fen: &str) -> String {
    fen.split_whitespace().take(4).collect::<Vec<_>>().join(" ")
}

#[async_trait]
impl Evaluator for ScriptedEngine {
    async fn analyze(
        &self,
        request: &AnalysisRequest,
        cancel: &CancellationToken,
    ) -> Result<PositionAnalysis, EngineError> {
        let allowed = |pv: &[String]| {
            request
                .searchmoves
                .as_ref()
                .is_none_or(|moves| pv.first().is_some_and(|m| moves.contains(m)))
        };
        let lines: Vec<_> = self
            .scripts
            .get(&position_key(&request.fen))
            .map(|lines| lines.iter().filter(|(_, pv)| allowed(pv)).collect())
            .unwrap_or_default();
        if lines.is_empty() {
            return self.fallback.analyze(request, cancel).await;
        }
        Ok(PositionAnalysis {
            lines: lines
                .into_iter()
                .take(usize::from(request.multipv.max(1)))
                .enumerate()
                .map(|(i, (cp, pv))| EngineLine {
                    rank: i as u8 + 1,
                    eval: Eval::Cp { cp: *cp },
                    depth: 1,
                    pv: pv.clone(),
                })
                .collect(),
        })
    }

    fn name(&self) -> String {
        "ScriptedEngine".to_owned()
    }

    fn parallelism(&self) -> usize {
        2
    }
}

pub fn side_name(side: Side) -> &'static str {
    match side {
        Side::White => "white",
        Side::Black => "black",
    }
}
