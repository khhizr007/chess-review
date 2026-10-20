//! The coach inside the review pipeline, on the deterministic fake engines.

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::{FakeEngine, ScriptedEngine};
use review_core::{Classification, ReviewConfig};
use review_server::analysis::model::ReviewResult;
use review_server::analysis::{ReviewRequest, run_review};
use review_server::coach::circ::Circ;
use review_server::coach::detect::Tag;
use review_server::coach::{explains, is_good};
use review_server::engine::{Evaluator, SearchLimit};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

async fn review_pgn(pgn: &str, engine: Arc<dyn Evaluator>) -> ReviewResult {
    review_with(pgn, engine, ReviewConfig::default()).await
}

async fn review_with(pgn: &str, engine: Arc<dyn Evaluator>, config: ReviewConfig) -> ReviewResult {
    let game = parse_pgn(pgn).unwrap().remove(0).unwrap();
    run_review(
        ReviewRequest {
            game,
            limit: SearchLimit::Depth { depth: 1 },
            multipv: 3,
            config,
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

async fn review(fixture: &str) -> ReviewResult {
    review_pgn(&common::fixture(fixture), Arc::new(FakeEngine::default())).await
}

#[tokio::test]
async fn scholars_mate_blunder_is_explained_by_the_mate_it_allows() {
    let r = review("scholars_mate.pgn").await;
    let nf6 = &r.plies[5];
    assert_eq!(nf6.classification, Some(Classification::Blunder));
    let main = nf6.coach.main_event().expect("the blunder is explained");
    assert_eq!(main.tag, Tag::Checkmate);
    assert_eq!(main.chain, vec![Circ::Permits]);
    assert_eq!(nf6.coach.text()[0], "After Nf6, Qxf7# is checkmate.");
    assert_eq!(nf6.coach.show_lines[0].moves[0].san, "Qxf7#");
    // The factual sentences stay for the details drawer.
    assert!(!nf6.explanation.is_empty());
    // Book moves get no coach note; a forced move gets one sentence.
    for p in r.plies.iter().filter(|p| !explains(p.classification)) {
        let expected = usize::from(p.classification == Some(Classification::Forced));
        assert_eq!(p.coach.sentences.len(), expected, "ply {}", p.ply);
        assert!(p.coach.events.is_empty(), "ply {}", p.ply);
    }
}

#[tokio::test]
async fn a_missed_mate_names_the_mate() {
    let r = review("missed_mate.pgn").await;
    let qf3 = &r.plies[6];
    assert_eq!(qf3.classification, Some(Classification::Miss));
    let main = qf3.coach.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::Checkmate, vec![Circ::Misses])
    );
    assert_eq!(
        qf3.coach.text(),
        vec!["White missed Qxf7#, which is checkmate."]
    );
}

#[tokio::test]
async fn every_explained_sentence_is_backed_by_an_event_with_a_line() {
    for fixture in [
        "scholars_mate.pgn",
        "missed_mate.pgn",
        "opera_game.pgn",
        "back_rank.pgn",
    ] {
        let r = review(fixture).await;
        for p in &r.plies {
            for s in &p.coach.sentences {
                if let Some(i) = s.event {
                    let e = &p.coach.events[i as usize];
                    assert!(
                        !e.line.is_empty(),
                        "{fixture} ply {}: event without a line",
                        p.ply
                    );
                }
            }
            if let Some(main) = p.coach.main_event() {
                assert!(explains(p.classification));
                assert!(!p.coach.show_lines.is_empty());
                let marked = p.coach.sentences[0]
                    .fragments
                    .iter()
                    .any(|f| !f.arrows.is_empty() || !f.squares.is_empty());
                assert!(
                    marked,
                    "{fixture} ply {}: {:?} has no board marks",
                    p.ply, main.tag
                );
            }
        }
    }
}

#[tokio::test]
async fn a_scripted_fork_is_explained_with_its_line() {
    // 1...Ke8?? walks into Nc7+.
    let pgn = r#"[SetUp "1"]
[FEN "r2k4/8/8/1N6/8/8/8/4K3 b - - 0 1"]
[WhiteElo "1500"]
[BlackElo "1500"]

1... Ke8 2. Nc7+ Kd7 3. Nxa8 *"#;
    let engine = ScriptedEngine::default()
        .script(
            "r2k4/8/8/1N6/8/8/8/4K3 b - - 0 1",
            &[(0, &["d8c8", "b5a7", "c8b7"])],
        )
        .script(
            "r3k3/8/8/1N6/8/8/8/4K3 w - - 1 2",
            &[(500, &["b5c7", "e8d7", "c7a8"])],
        );
    let r = review_pgn(pgn, Arc::new(engine)).await;
    let ke8 = &r.plies[0];
    assert!(explains(ke8.classification), "{:?}", ke8.classification);
    let main = ke8.coach.main_event().unwrap();
    assert_eq!(main.tag, Tag::Fork);
    assert_eq!(main.chain, vec![Circ::Permits, Circ::WinsMaterial]);
    assert_eq!(main.line, vec!["b5c7", "e8d7", "c7a8"]);
    assert_eq!(
        ke8.coach.text()[0],
        "Ke8 allows Nc7+, which forks Black's king and rook on a8, winning a rook."
    );
}

// 6.Bc4 from the Opera game, with White's h-pawn gone so the position is out
// of book.
const BEFORE_BC4: &str = "rn1qkbnr/ppp2ppp/8/4p3/4P3/5Q2/PPP2PP1/RNB1KB1R w KQkq - 0 6";

#[tokio::test]
async fn a_null_move_search_finds_the_threat_of_a_good_move() {
    let pgn = format!(
        r#"[SetUp "1"]
[FEN "{BEFORE_BC4}"]
[WhiteElo "1500"]
[BlackElo "1500"]

6. Bc4 Nf6 *"#
    );
    // The null-move search after Bc4 (White to move again) is left to the
    // fake engine, which finds Qxf7#.
    let engine = ScriptedEngine::default()
        .script(BEFORE_BC4, &[(150, &["f1c4", "g8f6", "f3b3"])])
        .script(
            "rn1qkbnr/ppp2ppp/8/4p3/2B1P3/5Q2/PPP2PP1/RNB1K2R b KQkq - 1 6",
            &[(150, &["g8f6", "f3b3"])],
        );
    let r = review_pgn(&pgn, Arc::new(engine)).await;
    let bc4 = &r.plies[0];
    assert_eq!(bc4.classification, Some(Classification::Best));
    let main = bc4.coach.main_event().expect("the good move is explained");
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::Checkmate, vec![Circ::Threatens])
    );
    assert_eq!(main.line, vec!["f3f7"]);
    assert_eq!(
        bc4.coach.text(),
        vec!["Bc4 threatens Qxf7#, which is checkmate."]
    );
    assert_eq!(bc4.coach.show_lines[0].label, "Show the threat");
}

#[tokio::test]
async fn a_scripted_threat_search_before_the_move_is_stopped() {
    let fen = "3rk3/8/8/8/3N4/8/2P5/4K3 w - - 0 1";
    let pgn = format!(
        r#"[SetUp "1"]
[FEN "{fen}"]
[WhiteElo "1500"]
[BlackElo "1500"]

1. c3 Ke7 *"#
    );
    // The opponent's threat before c3 comes from the depth-10 null-move search.
    let engine = ScriptedEngine::default()
        .script(fen, &[(300, &["c2c3", "e8e7"])])
        .script("3rk3/8/8/8/3N4/2P5/8/4K3 b - - 0 1", &[(300, &["e8e7"])])
        .script_at_depth(
            "3rk3/8/8/8/3N4/8/2P5/4K3 b - - 0 1",
            10,
            &[(-300, &["d8d4", "e1e2"])],
        );
    let r = review_pgn(&pgn, Arc::new(engine)).await;
    let c3 = &r.plies[0];
    assert!(is_good(c3.classification), "{:?}", c3.classification);
    // The defence is said, and the stopped threat is among the events.
    let main = c3.coach.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::DefendsPiece, vec![Circ::Base])
    );
    assert_eq!(c3.coach.text(), vec!["c3 defends White's knight on d4."]);
    let stopped = c3
        .coach
        .events
        .iter()
        .find(|e| e.chain[0] == Circ::Prevents)
        .expect("the stopped threat");
    assert_eq!(stopped.tag, Tag::FreePiece);
    assert_eq!(stopped.line, vec!["d8d4", "e1e2"]);
}

#[tokio::test]
async fn the_threat_search_can_be_turned_off() {
    let pgn = common::fixture("opera_game.pgn");
    let mut calls = Vec::new();
    for (depth, after_best) in [(0, true), (10, false), (10, true)] {
        let engine = Arc::new(FakeEngine::default());
        let mut config = ReviewConfig::default();
        config.coach.threat_search_depth = depth;
        config.coach.best_threat_search = after_best;
        let r = review_with(&pgn, engine.clone(), config).await;
        let threats = r
            .plies
            .iter()
            .flat_map(|p| &p.coach.events)
            .filter(|e| matches!(e.chain[0], Circ::Threatens | Circ::Prevents))
            .count();
        assert_eq!(
            threats == 0,
            depth == 0,
            "depth {depth}: {threats} threat events"
        );
        calls.push(engine.calls.load(Ordering::SeqCst));
    }
    // One null-move search per position next to a move that is not book or
    // forced, and one after the best move of each of those moves that was not
    // the best move.
    assert!(calls[1] > calls[0], "{calls:?}");
    let r = review("opera_game.pgn").await;
    let coached: Vec<_> = r
        .plies
        .iter()
        .filter(|p| !p.is_book && p.classification != Some(Classification::Forced))
        .collect();
    let n = coached.len() as u32;
    assert!(calls[1] - calls[0] <= n + 1, "{calls:?} for {n} plies");
    let not_best = coached.iter().filter(|p| !p.played_is_best).count() as u32;
    assert!(
        calls[2] - calls[1] <= not_best,
        "{calls:?}: {not_best} plies not the best move"
    );
}
