//! Brilliant detection end to end, on scripted engine lines.

mod common;

use std::sync::Arc;

use common::ScriptedEngine;
use review_core::{Classification, ReviewConfig};
use review_server::analysis::model::ReviewResult;
use review_server::analysis::{ReviewRequest, run_review};
use review_server::engine::{Evaluator, SearchLimit};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

/// Castled black king, white bishop on d3 aiming at h7 (Greek gift).
const GREEK_GIFT: &str = "r1bq1rk1/pppn1ppp/4p3/3pP3/1b1P4/2NB1N2/PPP2PPP/R1BQK2R w KQ - 0 8";

async fn review(movetext: &str, engine: ScriptedEngine) -> ReviewResult {
    let pgn = format!(
        "[White \"W\"]\n[Black \"B\"]\n[WhiteElo \"2000\"]\n[BlackElo \"2000\"]\n\
         [SetUp \"1\"]\n[FEN \"{GREEK_GIFT}\"]\n\n{movetext} *\n"
    );
    let game = parse_pgn(&pgn).unwrap().remove(0).unwrap();
    let evaluator: Arc<dyn Evaluator> = Arc::new(engine);
    run_review(
        ReviewRequest {
            game,
            limit: SearchLimit::Depth { depth: 1 },
            multipv: 3,
            config: ReviewConfig::default(),
        },
        evaluator,
        common::openings(),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .unwrap()
}

fn first_move(r: &ReviewResult) -> Option<Classification> {
    r.plies[0].classification
}

#[tokio::test]
async fn hanging_piece_sacrifice_rated_best_is_brilliant() {
    let engine = ScriptedEngine::default().script(
        GREEK_GIFT,
        &[
            (150, &["d3h7", "g8h7", "f3g5"]),
            (-30, &["c1d2"]),
            (-40, &["e1g1"]),
        ],
    );
    let r = review("8. Bxh7+ Kxh7", engine).await;
    assert_eq!(first_move(&r), Some(Classification::Brilliant));
}

#[tokio::test]
async fn sacrifice_that_is_not_the_best_move_is_not_brilliant() {
    let engine = ScriptedEngine::default().script(
        GREEK_GIFT,
        &[(150, &["c1d2"]), (140, &["d3h7", "g8h7"]), (-40, &["e1g1"])],
    );
    let r = review("8. Bxh7+ Kxh7", engine).await;
    assert_eq!(first_move(&r), Some(Classification::Excellent));
}

#[tokio::test]
async fn sacrifice_when_already_winning_big_is_not_brilliant() {
    let engine = ScriptedEngine::default().script(
        GREEK_GIFT,
        &[
            (500, &["d3h7", "g8h7", "f3g5"]),
            (200, &["c1d2"]),
            (150, &["e1g1"]),
        ],
    );
    let r = review("8. Bxh7+ Kxh7", engine).await;
    assert!(r.plies[0].ep_before >= 0.90);
    assert_eq!(first_move(&r), Some(Classification::Best));
}

#[tokio::test]
async fn sacrifice_without_a_clearly_worse_alternative_is_not_brilliant() {
    let engine = ScriptedEngine::default().script(
        GREEK_GIFT,
        &[(150, &["d3h7", "g8h7"]), (140, &["c1d2"]), (130, &["e1g1"])],
    );
    let r = review("8. Bxh7+ Kxh7", engine).await;
    assert_eq!(first_move(&r), Some(Classification::Best));
}

#[tokio::test]
async fn trade_the_opponent_starts_later_is_not_brilliant() {
    // a3 Bxc3+ bxc3: the line drops a piece for one ply, but nothing is given up.
    let engine = ScriptedEngine::default().script(
        GREEK_GIFT,
        &[
            (60, &["a2a3", "b4c3", "b2c3"]),
            (40, &["c1d2"]),
            (30, &["e1g1"]),
        ],
    );
    let r = review("8. a3 Bxc3+ 9. bxc3", engine).await;
    assert_eq!(first_move(&r), Some(Classification::Best));
}

#[tokio::test]
async fn quiet_best_move_is_never_brilliant() {
    let engine = ScriptedEngine::default().script(
        GREEK_GIFT,
        &[(60, &["e1g1"]), (-100, &["c1d2"]), (-120, &["a2a3"])],
    );
    let r = review("8. O-O", engine).await;
    assert_ne!(first_move(&r), Some(Classification::Brilliant));
}
