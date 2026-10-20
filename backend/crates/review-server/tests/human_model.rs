//! The optional human-move model: its sidecar protocol, and how its odds
//! reach the coach's wording through the review pipeline.

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use async_trait::async_trait;
use common::FakeEngine;
use review_core::{Classification, ReviewConfig};
use review_server::analysis::model::ReviewResult;
use review_server::analysis::{ReviewRequest, run_review, run_review_with_model};
use review_server::engine::SearchLimit;
use review_server::human::{HumanModel, HumanModelError, MaiaSidecar, MoveQuery};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

/// Answers every query with the same odds for the played and the best move.
struct FixedOdds {
    played: f32,
    best: f32,
    calls: AtomicU32,
    fail: bool,
}

impl FixedOdds {
    fn new(played: f32, best: f32) -> FixedOdds {
        FixedOdds {
            played,
            best,
            calls: AtomicU32::new(0),
            fail: false,
        }
    }
}

#[async_trait]
impl HumanModel for FixedOdds {
    async fn odds(&self, queries: &[MoveQuery]) -> Result<Vec<Vec<f32>>, HumanModelError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.fail {
            return Err(HumanModelError::Failed("broken".into()));
        }
        Ok(queries
            .iter()
            .map(|_| vec![self.played, self.best])
            .collect())
    }

    fn name(&self) -> String {
        "fixed".into()
    }
}

fn request(fixture: &str) -> ReviewRequest {
    ReviewRequest {
        game: parse_pgn(&common::fixture(fixture))
            .unwrap()
            .remove(0)
            .unwrap(),
        limit: SearchLimit::Depth { depth: 1 },
        multipv: 3,
        config: ReviewConfig::default(),
        player_side: None,
    }
}

async fn review(model: Option<Arc<dyn HumanModel>>) -> ReviewResult {
    run_review_with_model(
        request("scholars_mate.pgn"),
        Arc::new(FakeEngine::default()),
        model,
        common::openings(),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn a_natural_blunder_leads_with_the_safety_check() {
    // 3...Nf6?? is the move most players of this level would play.
    let r = review(Some(Arc::new(FixedOdds::new(0.5, 0.2)))).await;
    let nf6 = &r.plies[5];
    assert_eq!(nf6.classification, Some(Classification::Blunder));
    assert_eq!(
        nf6.coach.text()[0],
        "Nf6 looks natural, but check the reply first: Qxf7# is checkmate."
    );
}

#[tokio::test]
async fn without_the_model_the_wording_is_unchanged() {
    let plain = run_review(
        request("scholars_mate.pgn"),
        Arc::new(FakeEngine::default()),
        common::openings(),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .unwrap();
    assert_eq!(
        plain.plies[5].coach.text()[0],
        "After Nf6, Qxf7# is checkmate."
    );
    // A model that fails is logged and left out.
    let broken = Arc::new(FixedOdds {
        fail: true,
        ..FixedOdds::new(0.5, 0.2)
    });
    let r = review(Some(broken.clone())).await;
    assert_eq!(broken.calls.load(Ordering::Relaxed), 1);
    assert_eq!(r.plies[5].coach, plain.plies[5].coach);
}

fn fake_sidecar() -> String {
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/fake_maia.sh");
    format!("sh {}", script.display())
}

#[cfg(unix)]
#[tokio::test]
async fn the_sidecar_answers_line_by_line() {
    let model = MaiaSidecar::start(&fake_sidecar()).await.unwrap();
    assert_eq!(model.name(), "fake Maia");
    let q = |fen: &str| MoveQuery {
        fen: fen.to_owned(),
        moves: vec!["e2e4".into(), "d2d4".into()],
        elo_self: 1200,
        elo_oppo: 1300,
    };
    let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let odds = model.odds(&[q(start), q(start)]).await.unwrap();
    assert_eq!(odds, vec![vec![0.5, 0.25], vec![0.5, 0.25]]);
    // A second batch on the same process.
    assert_eq!(model.odds(&[q(start)]).await.unwrap().len(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn a_sidecar_that_cannot_start_is_an_error() {
    assert!(MaiaSidecar::start("").await.is_err());
    assert!(
        MaiaSidecar::start("/nonexistent/maia-sidecar")
            .await
            .is_err()
    );
    // A process that exits without a ready line.
    assert!(MaiaSidecar::start("true").await.is_err());
}
