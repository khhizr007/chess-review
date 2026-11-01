//! What the coach says is true on the board (`coach::verify`): every claim
//! of every note, on the fixture games and a sample of the calibration games,
//! reviewed by the deterministic fake engine from each side's point of view.
//! And the checker is not blind: wrong notes are found wrong.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use common::FakeEngine;
use review_core::{ReviewConfig, Side};
use review_server::analysis::model::{PlyReview, ReviewResult};
use review_server::analysis::{ReviewRequest, run_review};
use review_server::board;
use review_server::coach::verify::mutate::{Mutation, Target, mutations};
use review_server::coach::verify::{Place, Report, VerifyInput, verify};
use review_server::coach::{CoachNote, Style};
use review_server::engine::SearchLimit;
use review_server::pgn::parse_pgn;
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess};
use tokio_util::sync::CancellationToken;

/// Fixture games with something to say (not the invalid one).
const FIXTURES: [&str; 10] = [
    "scholars_mate.pgn",
    "opera_game.pgn",
    "missed_mate.pgn",
    "back_rank.pgn",
    "stalemate.pgn",
    "special_moves.pgn",
    "chess960.pgn",
    "agreed_draw.pgn",
    "sparse_metadata.pgn",
    "multi_games.pgn",
];

/// Calibration games in the sample (`tests/fixtures/calibration-sample`: the first 16 by id of the
/// calibration set, rated online games).
const CALIBRATION_SAMPLE: usize = 16;

async fn reviews(pgn: &str, player_side: Option<Side>) -> Vec<ReviewResult> {
    let mut out = Vec::new();
    // multi_games.pgn holds an illegal game on purpose.
    for game in parse_pgn(pgn).unwrap().into_iter().filter_map(Result::ok) {
        out.push(
            run_review(
                ReviewRequest {
                    game,
                    limit: SearchLimit::Depth { depth: 1 },
                    multipv: 3,
                    config: ReviewConfig::default(),
                    player_side,
                },
                Arc::new(FakeEngine::default()),
                common::openings(),
                CancellationToken::new(),
                Arc::new(|_, _| {}),
            )
            .await
            .unwrap(),
        );
    }
    out
}

fn games() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = FIXTURES
        .iter()
        .map(|f| ((*f).to_owned(), common::fixture(f)))
        .collect();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/calibration-sample");
    let mut pgns: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "pgn"))
        .collect();
    pgns.sort();
    for p in pgns.into_iter().take(CALIBRATION_SAMPLE) {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        out.push((name, std::fs::read_to_string(&p).unwrap()));
    }
    out
}

/// Checks one ply's note (or a changed copy of it).
fn check(r: &ReviewResult, i: usize, note: &CoachNote) -> Report {
    let mode = if r.game.chess960 {
        CastlingMode::Chess960
    } else {
        CastlingMode::Standard
    };
    let position = |p: &PlyReview| -> Chess {
        p.fen_before
            .parse::<Fen>()
            .unwrap()
            .into_position(mode)
            .unwrap()
    };
    let p = &r.plies[i];
    let before = position(p);
    let played = board::parse_uci(&before, &p.uci).unwrap();
    let previous = i.checked_sub(1).map(|j| {
        let q = &r.plies[j];
        board::parse_uci(&position(q), &q.uci).unwrap()
    });
    let best: Vec<String> = p
        .lines
        .first()
        .map(|l| l.moves.iter().map(|m| m.uci.clone()).collect())
        .unwrap_or_default();
    verify(&VerifyInput {
        note,
        before: &before,
        played,
        previous,
        best: &best,
        style: Style::for_player(r.settings.player_side, p.mover),
    })
}

#[tokio::test]
async fn every_claim_the_coach_makes_is_true() {
    let mut false_claims = Vec::new();
    let mut sentences = 0;
    for (name, pgn) in games() {
        for side in [None, Some(Side::White), Some(Side::Black)] {
            for r in reviews(&pgn, side).await {
                for (i, p) in r.plies.iter().enumerate() {
                    let report = check(&r, i, &p.coach);
                    sentences += report.sentences;
                    for f in report.failures {
                        false_claims.push(format!(
                            "{name} ({side:?}) ply {} {}: {:?} at {:?}: \"{}\" ({}) in {:?}",
                            p.ply,
                            p.san,
                            f.kind,
                            f.place,
                            f.claim,
                            f.detail,
                            p.coach.text()
                        ));
                    }
                }
            }
        }
    }
    assert!(sentences > 500, "only {sentences} sentences checked");
    assert!(false_claims.is_empty(), "{}", false_claims.join("\n"));
}

#[tokio::test]
async fn the_checker_finds_wrong_notes_wrong() {
    // Mutations tried and caught, per kind.
    let mut tally: BTreeMap<Mutation, (usize, usize)> = BTreeMap::new();
    let mut missed = Vec::new();
    for (name, pgn) in games().into_iter().take(FIXTURES.len()) {
        for r in reviews(&pgn, None).await {
            for (i, p) in r.plies.iter().enumerate() {
                for (m, target, note) in mutations(&p.coach) {
                    let place = match target {
                        Target::Sentence(s) => Place::Sentence(s),
                        Target::Narration { line, ply } => Place::Narration { line, ply },
                    };
                    let caught = check(&r, i, &note).fails_at(place);
                    let t = tally.entry(m).or_insert((0, 0));
                    t.0 += 1;
                    t.1 += usize::from(caught);
                    if !caught && matches!(m, Mutation::Move | Mutation::Narration) {
                        missed.push(format!("{name} ply {} {m:?}: {:?}", p.ply, note.text()));
                    }
                }
            }
        }
    }
    // A different move, or a narration of another move, is never true.
    assert!(missed.is_empty(), "{}", missed.join("\n"));
    for m in [
        Mutation::Move,
        Mutation::Narration,
        Mutation::Square,
        Mutation::Owner,
    ] {
        let (tried, caught) = tally.get(&m).copied().unwrap_or((0, 0));
        assert!(tried > 0, "no {m:?} mutation tried");
        // A pawn moved one file may meet a pawn of the same side there.
        assert!(
            caught * 10 >= tried * 9,
            "{m:?}: caught {caught} of {tried}"
        );
    }
}
