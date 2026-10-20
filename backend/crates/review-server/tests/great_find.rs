//! Great Find end to end, on scripted engine lines.

mod common;

use std::sync::Arc;

use common::ScriptedEngine;
use review_core::{Classification, ReviewConfig};
use review_server::analysis::{ReviewRequest, run_review};
use review_server::engine::{Evaluator, SearchLimit};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

const START: &str = "r1bq1rk1/pppn1ppp/4p3/3pP3/1b1P4/2NB1N2/PPP2PPP/R1BQK2R w KQ - 0 8";
/// After 8.h3 h6: white to move.
const CANDIDATE: &str = "r1bq1rk1/pppn1pp1/4p2p/3pP3/1b1P4/2NB1N1P/PPP2PP1/R1BQK2R w KQ - 0 9";
/// Castling is the only good move: the alternatives lose about 0.25 EP.
const LINES: &[(i32, &[&str])] = &[
    (60, &["e1g1", "b4c3"]),
    (-150, &["c1d2"]),
    (-160, &["a2a3"]),
];

async fn castling_label(engine: ScriptedEngine, easy_search_depth: u32) -> Option<Classification> {
    let pgn = format!(
        "[White \"W\"]\n[Black \"B\"]\n[WhiteElo \"2000\"]\n[BlackElo \"2000\"]\n\
         [SetUp \"1\"]\n[FEN \"{START}\"]\n\n8. h3 h6 9. O-O *\n"
    );
    let mut config = ReviewConfig::default();
    config.great_find.easy_search_depth = easy_search_depth;
    let evaluator: Arc<dyn Evaluator> = Arc::new(engine);
    let r = run_review(
        ReviewRequest {
            game: parse_pgn(&pgn).unwrap().remove(0).unwrap(),
            limit: SearchLimit::Depth { depth: 18 },
            multipv: 3,
            config,
            player_side: None,
        },
        evaluator,
        common::openings(),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .unwrap();
    assert_eq!(r.plies[2].san, "O-O");
    r.plies[2].classification
}

#[tokio::test]
async fn only_good_move_is_a_great_find() {
    let engine = ScriptedEngine::default().script(CANDIDATE, LINES);
    assert_eq!(
        castling_label(engine, 0).await,
        Some(Classification::GreatFind)
    );
}

#[tokio::test]
async fn only_good_move_missed_by_a_shallow_search_is_a_great_find() {
    let engine = ScriptedEngine::default()
        .script(CANDIDATE, LINES)
        .script_at_depth(CANDIDATE, 4, &[(20, &["c1d2"]), (10, &["e1g1"])]);
    assert_eq!(
        castling_label(engine, 4).await,
        Some(Classification::GreatFind)
    );
}

#[tokio::test]
async fn only_good_move_a_shallow_search_finds_is_easy() {
    // The depth-4 search uses the same lines, so it already prefers castling.
    let engine = ScriptedEngine::default().script(CANDIDATE, LINES);
    assert_eq!(castling_label(engine, 4).await, Some(Classification::Best));
}
