//! Judging sideline moves (docs/explore-mode.md §5.5): the same rules as the review, on lines the
//! client sends.

mod common;

use std::sync::Arc;

use common::FakeEngine;
use review_core::{Eval, ReviewConfig};
use review_server::analysis::model::ReviewResult;
use review_server::analysis::sideline::{JudgeError, SidelineMove, judge_sideline};
use review_server::analysis::{ReviewRequest, run_review};
use review_server::engine::{AnalysisRequest, EngineLine, Evaluator, SearchLimit};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

async fn review(fixture: &str) -> ReviewResult {
    let game = parse_pgn(&common::fixture(fixture))
        .unwrap()
        .remove(0)
        .unwrap();
    let engine: Arc<dyn Evaluator> = Arc::new(FakeEngine::default());
    run_review(
        ReviewRequest {
            game,
            limit: SearchLimit::Depth { depth: 1 },
            multipv: 3,
            config: ReviewConfig::default(),
            player_side: None,
        },
        engine,
        common::openings(),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .unwrap()
}

/// The engine's full lines for a position, as the review searched it (the fake engine is deterministic).
async fn lines(r: &ReviewResult, fen: &str) -> Vec<EngineLine> {
    FakeEngine::default()
        .analyze(
            &AnalysisRequest {
                fen: fen.to_owned(),
                limit: SearchLimit::Depth { depth: 1 },
                multipv: 3,
                chess960: r.game.chess960,
                searchmoves: None,
            },
            &CancellationToken::new(),
        )
        .await
        .unwrap()
        .lines
}

/// Game move `i` asked as a one-move sideline from ply `i`, with the engine's lines around it.
async fn as_sideline(r: &ReviewResult, i: usize) -> SidelineMove<'_> {
    let ply = &r.plies[i];
    let after = match r.plies.get(i + 1) {
        Some(next) => lines(r, &next.fen_before).await.into_iter().next(),
        // The final position: searched by the review unless the game is over there.
        None => lines(r, &ply.fen_after).await.into_iter().next(),
    };
    SidelineMove {
        from_ply: i as u32,
        moves: std::slice::from_ref(&ply.uci),
        before: lines(r, &ply.fen_before).await,
        after,
        previous_ep_lost: None,
    }
}

const FIXTURES: [&str; 9] = [
    "agreed_draw.pgn",
    "back_rank.pgn",
    "chess960.pgn",
    "missed_mate.pgn",
    "opera_game.pgn",
    "scholars_mate.pgn",
    "sparse_metadata.pgn",
    "special_moves.pgn",
    "stalemate.pgn",
];

#[tokio::test]
async fn a_game_move_asked_as_a_sideline_is_judged_as_the_review_judged_it() {
    let openings = common::openings();
    let mut judged = 0;
    for fixture in FIXTURES {
        let r = review(fixture).await;
        for (i, ply) in r.plies.iter().enumerate() {
            let j = judge_sideline(&r, &openings, &as_sideline(&r, i).await)
                .unwrap_or_else(|e| panic!("{fixture} ply {}: {e}", ply.ply));
            let at = format!("{fixture} ply {} {}", ply.ply, ply.san);
            assert_eq!(j.classification, ply.classification, "{at}");
            assert_eq!(j.miss_kind, ply.miss_kind, "{at}");
            assert_eq!(j.rule, ply.rule, "{at}");
            assert!((j.ep_lost - ply.ep_lost).abs() < 1e-12, "{at}");
            assert!((j.ep_after - ply.ep_after).abs() < 1e-12, "{at}");
            assert_eq!(j.played_is_best, ply.played_is_best, "{at}");
            assert_eq!(j.show_best_move, ply.show_best_move, "{at}");
            assert_eq!(j.best_move, ply.best_move, "{at}");
            assert_eq!(j.explanation, ply.explanation, "{at}");
            judged += 1;
        }
    }
    assert!(judged > 100, "only {judged} moves judged");
}

#[tokio::test]
async fn a_later_sideline_move_uses_the_previous_sideline_moves_loss() {
    let openings = common::openings();
    let r = review("opera_game.pgn").await;
    // 1. e4 e5 2. Nf3, then 2... Nc6 instead of 2... d6, then 3. Bb5.
    let moves = ["b8c6".to_owned(), "f1b5".to_owned()];
    let before = |eval_cp: i32, pv: &[&str]| {
        vec![EngineLine {
            rank: 1,
            eval: Eval::Cp { cp: eval_cp },
            depth: 18,
            pv: pv.iter().map(|m| (*m).to_owned()).collect(),
        }]
    };
    let first = judge_sideline(
        &r,
        &openings,
        &SidelineMove {
            from_ply: 3,
            moves: &moves[..1],
            before: before(30, &["b8c6", "f1b5"]),
            after: None,
            previous_ep_lost: None,
        },
    )
    .unwrap();
    assert!(first.played_is_best);
    let second = judge_sideline(
        &r,
        &openings,
        &SidelineMove {
            from_ply: 3,
            moves: &moves,
            before: before(30, &["f1b5", "a7a6"]),
            after: None,
            previous_ep_lost: Some(first.ep_lost),
        },
    )
    .unwrap();
    assert!(second.played_is_best);
    assert_eq!(second.best_move.unwrap().san, "Bb5");
}

#[tokio::test]
async fn bad_sidelines_are_refused() {
    let openings = common::openings();
    let r = review("scholars_mate.pgn").await;
    let err = |req: SidelineMove<'_>| judge_sideline(&r, &openings, &req).map(|_| ()).unwrap_err();
    assert!(judge_sideline(&r, &openings, &as_sideline(&r, 2).await).is_ok());

    let illegal = ["e1e3".to_owned()];
    let req = SidelineMove {
        moves: &illegal,
        ..as_sideline(&r, 2).await
    };
    assert_eq!(err(req), JudgeError::IllegalMove("e1e3".into()));
    assert_eq!(
        err(SidelineMove {
            from_ply: 99,
            ..as_sideline(&r, 2).await
        }),
        JudgeError::BadPly(99)
    );
    assert_eq!(
        err(SidelineMove {
            moves: &[],
            ..as_sideline(&r, 2).await
        }),
        JudgeError::BadLength
    );
    assert_eq!(
        err(SidelineMove {
            before: Vec::new(),
            ..as_sideline(&r, 2).await
        }),
        JudgeError::BadLines
    );
    let mut wrong = as_sideline(&r, 2).await.before;
    wrong[0].pv = vec!["a1a8".into()];
    assert_eq!(
        err(SidelineMove {
            before: wrong,
            ..as_sideline(&r, 2).await
        }),
        JudgeError::BadLines
    );

    // A move outside the lines needs the position after it.
    let other = ["g1f3".to_owned()];
    let req = SidelineMove {
        moves: &other,
        after: None,
        ..as_sideline(&r, 2).await
    };
    assert_eq!(err(req), JudgeError::MissingAfter);
}
