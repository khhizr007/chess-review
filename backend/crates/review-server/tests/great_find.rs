//! Great end to end, on scripted engine lines.

mod common;

use std::sync::Arc;

use common::ScriptedEngine;
use review_core::{Classification, ReviewConfig};
use review_server::analysis::model::ReviewResult;
use review_server::analysis::{ReviewRequest, run_review};
use review_server::engine::{Evaluator, SearchLimit};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

const START: &str = "r1bq1rk1/pppn1ppp/4p3/3pP3/1b1P4/2NB1N2/PPP2PPP/R1BQK2R w KQ - 0 8";
/// After 8.h3 h6: white to move.
const CANDIDATE: &str = "r1bq1rk1/pppn1pp1/4p2p/3pP3/1b1P4/2NB1N1P/PPP2PP1/R1BQK2R w KQ - 0 9";
/// Castling is the only good move: the alternatives lose about 0.25 EP.
const CASTLING: &[(i32, &[&str])] = &[
    (60, &["e1g1", "b4c3"]),
    (-150, &["c1d2"]),
    (-160, &["a2a3"]),
];
/// The same, with 9.a3 as the only good move.
const PAWN_MOVE: &[(i32, &[&str])] = &[
    (60, &["a2a3", "b4c3"]),
    (-150, &["c1d2"]),
    (-160, &["e1g1"]),
];

async fn review(engine: ScriptedEngine, pgn: &str, easy_search_depth: u32) -> ReviewResult {
    let mut config = ReviewConfig::default();
    config.great_find.easy_search_depth = easy_search_depth;
    let evaluator: Arc<dyn Evaluator> = Arc::new(engine);
    run_review(
        ReviewRequest {
            game: parse_pgn(pgn).unwrap().remove(0).unwrap(),
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
    .unwrap()
}

/// The label of White's ninth move (`san`) after 8.h3 h6.
async fn ninth_move_label(
    engine: ScriptedEngine,
    san: &str,
    easy_search_depth: u32,
) -> Option<Classification> {
    let pgn = format!(
        "[White \"W\"]\n[Black \"B\"]\n[WhiteElo \"2000\"]\n[BlackElo \"2000\"]\n\
         [SetUp \"1\"]\n[FEN \"{START}\"]\n\n8. h3 h6 9. {san} *\n"
    );
    let r = review(engine, &pgn, easy_search_depth).await;
    assert_eq!(r.plies[2].san, san);
    r.plies[2].classification
}

#[tokio::test]
async fn only_good_move_is_great() {
    let engine = ScriptedEngine::default().script(CANDIDATE, CASTLING);
    assert_eq!(
        ninth_move_label(engine, "O-O", 0).await,
        Some(Classification::GreatFind)
    );
}

#[tokio::test]
async fn castling_is_never_easy_to_find() {
    // The depth-4 search uses the same lines, so it already prefers castling.
    let engine = ScriptedEngine::default().script(CANDIDATE, CASTLING);
    assert_eq!(
        ninth_move_label(engine, "O-O", 4).await,
        Some(Classification::GreatFind)
    );
}

#[tokio::test]
async fn only_good_move_missed_by_a_shallow_search_is_great() {
    let engine = ScriptedEngine::default()
        .script(CANDIDATE, PAWN_MOVE)
        .script_at_depth(CANDIDATE, 4, &[(20, &["c1d2"]), (10, &["a2a3"])]);
    assert_eq!(
        ninth_move_label(engine, "a3", 4).await,
        Some(Classification::GreatFind)
    );
}

#[tokio::test]
async fn only_good_move_a_shallow_search_finds_is_easy() {
    let engine = ScriptedEngine::default().script(CANDIDATE, PAWN_MOVE);
    assert_eq!(
        ninth_move_label(engine, "a3", 4).await,
        Some(Classification::Best)
    );
}

/// 2...Qg5?? hangs the queen to the knight; the label of 3.Nxg5 with both players at `elo`.
async fn taking_the_hanging_queen(elo: u32) -> Option<Classification> {
    let pgn = format!(
        "[White \"W\"]\n[Black \"B\"]\n[WhiteElo \"{elo}\"]\n[BlackElo \"{elo}\"]\n\n\
         1. e4 e5 2. Nf3 Qg5 3. Nxg5 *\n"
    );
    let r = review(ScriptedEngine::default(), &pgn, 0).await;
    assert_eq!(r.plies[3].classification, Some(Classification::Blunder));
    assert_eq!(r.plies[4].san, "Nxg5");
    r.plies[4].classification
}

#[tokio::test]
async fn punishing_a_blunder_with_a_free_piece_is_great_for_new_players() {
    // Below 1000 Elo a free piece taken after the opponent's error is a key moment, and taking a
    // free piece is not "easy to find".
    assert_eq!(
        taking_the_hanging_queen(900).await,
        Some(Classification::GreatFind)
    );
}

#[tokio::test]
async fn taking_a_free_piece_is_easy_for_high_rated_players() {
    assert_eq!(
        taking_the_hanging_queen(2000).await,
        Some(Classification::Best)
    );
}
