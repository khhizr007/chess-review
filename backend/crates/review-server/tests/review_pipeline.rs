//! Review pipeline on a deterministic fake engine.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use common::FakeEngine;
use review_core::{Classification, Eval, ReviewConfig, Side};
use review_server::analysis::model::ReviewResult;
use review_server::analysis::{ReviewError, ReviewRequest, run_review};
use review_server::engine::{Evaluator, SearchLimit};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

fn request(fixture: &str) -> ReviewRequest {
    let game = parse_pgn(&common::fixture(fixture))
        .unwrap()
        .remove(0)
        .unwrap();
    ReviewRequest {
        game,
        limit: SearchLimit::Depth { depth: 1 },
        multipv: 3,
        config: ReviewConfig::default(),
    }
}

async fn review(fixture: &str) -> ReviewResult {
    let engine: Arc<dyn Evaluator> = Arc::new(FakeEngine::default());
    run_review(
        request(fixture),
        engine,
        common::openings(),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn scholars_mate_review() {
    let r = review("scholars_mate.pgn").await;
    assert_eq!(r.plies.len(), 7);
    assert_eq!(r.graph.len(), 8, "one graph point per position");
    assert_eq!(
        r.rating_used, 1400,
        "max(WhiteElo, BlackElo) + offset 200, clamped"
    );

    // Opening moves are in the public opening list.
    assert_eq!(r.plies[0].classification, Some(Classification::Book));
    assert!(r.plies[0].is_book);

    // 3...Nf6 walks into mate: a blunder that shows the best move.
    let nf6 = &r.plies[5];
    assert_eq!(nf6.san, "Nf6");
    assert_eq!(nf6.mover, Side::Black);
    assert_eq!(nf6.classification, Some(Classification::Blunder));
    assert!(nf6.show_best_move && nf6.best_move.is_some());
    assert!(nf6.is_critical);
    assert_eq!(
        nf6.eval_after,
        Eval::Mate {
            winner: Side::White,
            moves: 1
        }
    );

    // 4.Qxf7# is the engine's best move and ends the game.
    let mate = &r.plies[6];
    assert_eq!(mate.san, "Qxf7#");
    assert!(mate.played_is_best && mate.flags.checkmate);
    assert_eq!(mate.classification, Some(Classification::Best));
    assert_eq!(mate.accuracy, Some(100.0));
    assert_eq!(
        mate.eval_after,
        Eval::Mate {
            winner: Side::White,
            moves: 0
        }
    );

    // Summary is consistent with the per-move data.
    assert_eq!(r.black.blunders, 1);
    assert!(r.white.accuracy.overall.unwrap() > r.black.accuracy.overall.unwrap());
    assert!(r.critical_moments.contains(&6));
    assert_eq!(r.biggest_swings.first(), Some(&6));
    assert!(r.graph.last().unwrap().white_expected_points > 0.999);
}

#[tokio::test]
async fn missed_mate_in_one_is_a_miss() {
    let r = review("missed_mate.pgn").await;
    let qf3 = &r.plies[6];
    assert_eq!(qf3.san, "Qf3");
    assert_eq!(qf3.classification, Some(Classification::Miss));
    assert_eq!(qf3.miss_kind, Some(review_core::MissKind::Mate));
    assert_eq!(
        qf3.best_move.as_ref().map(|m| m.san.as_str()),
        Some("Qxf7#")
    );
    assert!(
        qf3.explanation
            .iter()
            .any(|s| s.contains("Qxf7# forces mate in 1 move")),
        "{:?}",
        qf3.explanation
    );
}

#[tokio::test]
async fn stalemate_final_position_is_not_searched() {
    let engine = Arc::new(FakeEngine::default());
    let evaluator: Arc<dyn Evaluator> = engine.clone();
    let mut req = request("stalemate.pgn");
    // Count only the main searches, not the shallow Great Find ones.
    req.config.great_find.easy_search_depth = 0;
    req.config.coach.threat_search_depth = 0;
    let positions = req.game.positions.len() as u32;
    let r = run_review(
        req,
        evaluator,
        common::openings(),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .unwrap();
    assert_eq!(engine.calls.load(Ordering::SeqCst), positions - 1);
    let last = r.plies.last().unwrap();
    assert!(last.flags.stalemate);
    assert_eq!(last.eval_after, Eval::DRAW);
    assert_eq!(r.graph.last().unwrap().white_expected_points, 0.5);
}

#[tokio::test]
async fn forced_moves_and_special_moves() {
    let r = review("special_moves.pgn").await;
    let ep = r.plies.iter().find(|p| p.flags.en_passant).unwrap();
    assert_eq!(ep.san, "exf6");
    let promo = r.plies.iter().find(|p| p.flags.promotion).unwrap();
    assert_eq!(promo.san, "gxh8=Q");
    let castle = r.plies.iter().find(|p| p.flags.castle).unwrap();
    assert_eq!(
        (castle.from.as_deref(), castle.to.as_str()),
        (Some("e1"), "g1")
    );
}

#[tokio::test]
async fn chess960_game_is_reviewed() {
    let r = review("chess960.pgn").await;
    assert!(r.game.chess960);
    assert_eq!(r.plies.len(), 4);
    assert!(r.plies.iter().all(|p| p.classification.is_some()));
}

#[tokio::test]
async fn unrated_game_uses_reference_rating_and_notes_it() {
    let r = review("sparse_metadata.pgn").await;
    assert_eq!(r.rating_used, 1000);
    assert!(r.notes.iter().any(|n| n.contains("no ratings")));
    assert!(
        r.plies
            .iter()
            .all(|p| p.classification != Some(Classification::GreatFind))
    );
}

#[tokio::test]
async fn progress_is_reported_and_cancellation_stops_the_review() {
    let engine = Arc::new(FakeEngine {
        delay: Some(Duration::from_millis(30)),
        ..Default::default()
    });
    let evaluator: Arc<dyn Evaluator> = engine.clone();
    let last_done = Arc::new(AtomicU32::new(0));
    let seen = Arc::clone(&last_done);
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        canceller.cancel();
    });
    let result = run_review(
        request("opera_game.pgn"),
        evaluator,
        common::openings(),
        cancel,
        Arc::new(move |done, _total| seen.store(done, Ordering::SeqCst)),
    )
    .await;
    assert_eq!(result.unwrap_err(), ReviewError::Cancelled);
    let done = last_done.load(Ordering::SeqCst);
    assert!(done > 0 && done < 34, "partial progress, got {done}");
}
