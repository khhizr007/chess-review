//! The coach inside the review pipeline, on the deterministic fake engines.

mod common;

use std::sync::Arc;

use common::{FakeEngine, ScriptedEngine};
use review_core::{Classification, ReviewConfig};
use review_server::analysis::model::ReviewResult;
use review_server::analysis::{ReviewRequest, run_review};
use review_server::coach::circ::Circ;
use review_server::coach::detect::Tag;
use review_server::coach::explains;
use review_server::engine::{Evaluator, SearchLimit};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

async fn review_pgn(pgn: &str, engine: Arc<dyn Evaluator>) -> ReviewResult {
    let game = parse_pgn(pgn).unwrap().remove(0).unwrap();
    run_review(
        ReviewRequest {
            game,
            limit: SearchLimit::Depth { depth: 1 },
            multipv: 3,
            config: ReviewConfig::default(),
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
    // Good and book moves get no coach note yet.
    for p in r.plies.iter().filter(|p| !explains(p.classification)) {
        assert!(p.coach.sentences.is_empty(), "ply {}", p.ply);
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
