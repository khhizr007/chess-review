//! Fact-checks what the coach says (`coach::verify`) and reports the rate of
//! false claims by kind.
//!
//! ```text
//! # a calibration run directory (or one of its game JSON files), as written by compare.py
//! cargo run --release -p review-server --example coach_verify -- tests/calibration/runs/<run> [--list]
//! # a PGN, reviewed here with a real engine
//! STOCKFISH_PATH=/path/to/stockfish \
//!   cargo run --release -p review-server --example coach_verify -- <game.pgn> [depth] [--list]
//! ```
//!
//! `--list` prints every false claim (otherwise the first 40) and the claims
//! left unchecked. `--mutate` also tests the checker: it changes one claim at
//! a time in each note (`coach::verify::mutate`) and reports how many of the
//! changes it finds false.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use review_core::{ReviewConfig, Side};
use review_server::analysis::{ReviewRequest, run_review};
use review_server::coach::verify::mutate::{Mutation, Target, mutations};
use review_server::coach::verify::{Kind, Place, PlyText, Report, verify_text};
use review_server::coach::{CoachNote, Style};
use review_server::config::AppConfig;
use review_server::engine::pool::EnginePool;
use review_server::engine::{Evaluator, SearchLimit};
use review_server::load_openings;
use review_server::pgn::parse_pgn;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

/// The parts of a stored review the checker needs.
#[derive(Deserialize)]
struct Stored {
    game_id: Option<String>,
    result: StoredResult,
}

#[derive(Deserialize)]
struct StoredResult {
    settings: Option<StoredSettings>,
    plies: Vec<StoredPly>,
}

#[derive(Deserialize)]
struct StoredSettings {
    player_side: Option<Side>,
}

#[derive(Deserialize)]
struct StoredPly {
    ply: u32,
    mover: Side,
    san: String,
    uci: String,
    fen_before: String,
    #[serde(default)]
    lines: Vec<StoredLine>,
    coach: CoachNote,
}

#[derive(Deserialize)]
struct StoredLine {
    moves: Vec<StoredMove>,
}

#[derive(Deserialize)]
struct StoredMove {
    uci: String,
}

struct Game {
    id: String,
    player: Option<Side>,
    plies: Vec<StoredPly>,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("coach_verify: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let list = args.iter().any(|a| a == "--list");
    let mutate = args.iter().any(|a| a == "--mutate");
    args.retain(|a| a != "--list" && a != "--mutate");
    let (path, depth) = match args.as_slice() {
        [p] => (PathBuf::from(p), 18),
        [p, d] => (
            PathBuf::from(p),
            d.parse::<u32>().map_err(|_| format!("bad depth {d}"))?,
        ),
        _ => {
            return Err(
                "usage: coach_verify <run dir | game.json | game.pgn> [depth] [--list]".to_owned(),
            );
        }
    };
    let games = if path.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        let mut games = Vec::new();
        for f in files {
            // Run directories also hold summaries; only game files have plies.
            if let Some(g) = load_json(&f)? {
                games.push(g);
            }
        }
        games
    } else if path.extension().is_some_and(|x| x == "pgn") {
        vec![review_pgn(&path, depth).await?]
    } else {
        load_json(&path)?.into_iter().collect()
    };
    if games.is_empty() {
        return Err(format!("no reviewed games in {}", path.display()));
    }

    let mut total = Report::default();
    // Sentences with a false claim of each kind.
    let mut sentences_by_kind: BTreeMap<Kind, usize> = BTreeMap::new();
    let mut shown = 0;
    let mut plies = 0;
    // Mutations tried and caught, per kind.
    let mut mutated: BTreeMap<Mutation, (usize, usize)> = BTreeMap::new();
    for g in &games {
        let style_for = |mover: Side| match g.player {
            Some(p) if p == mover => Style::You,
            Some(_) => Style::Opponent,
            None => Style::Neutral,
        };
        for (i, p) in g.plies.iter().enumerate() {
            plies += 1;
            let previous = i
                .checked_sub(1)
                .map(|j| (g.plies[j].fen_before.as_str(), g.plies[j].uci.as_str()));
            let best: Vec<String> = p
                .lines
                .first()
                .map(|l| l.moves.iter().map(|m| m.uci.clone()).collect())
                .unwrap_or_default();
            let check = |note: &CoachNote| {
                verify_text(&PlyText {
                    fen_before: &p.fen_before,
                    uci: &p.uci,
                    previous,
                    best: &best,
                    note,
                    style: style_for(p.mover),
                })
                .map_err(|e| format!("{} ply {}: {e}", g.id, p.ply))
            };
            let report = check(&p.coach)?;
            if mutate {
                for (m, target, note) in mutations(&p.coach) {
                    let place = match target {
                        Target::Sentence(s) => Place::Sentence(s),
                        Target::Narration { line, ply } => Place::Narration { line, ply },
                    };
                    // Only claims the checker finds true before the change.
                    if report.fails_at(place) {
                        continue;
                    }
                    let caught = check(&note)?.fails_at(place);
                    if list && !caught {
                        let text = match target {
                            Target::Sentence(s) => note.text()[s].clone(),
                            Target::Narration { line, ply } => {
                                note.show_lines[line].narration[ply].clone()
                            }
                        };
                        println!(
                            "{} ply {} {}: missed {} — {text}",
                            g.id,
                            p.ply,
                            p.san,
                            m.name()
                        );
                    }
                    let entry = mutated.entry(m).or_insert((0, 0));
                    entry.0 += 1;
                    entry.1 += usize::from(caught);
                }
            }
            let mut kinds: BTreeMap<usize, Vec<Kind>> = BTreeMap::new();
            for f in &report.failures {
                if let Place::Sentence(s) = f.place {
                    kinds.entry(s).or_default().push(f.kind);
                }
            }
            for mut ks in kinds.into_values() {
                ks.sort();
                ks.dedup();
                for k in ks {
                    *sentences_by_kind.entry(k).or_insert(0) += 1;
                }
            }
            if list {
                for u in &report.unchecked {
                    println!(
                        "{} ply {} {}: unchecked — \"{}\" — {}",
                        g.id, p.ply, p.san, u.claim, u.detail
                    );
                }
            }
            for f in &report.failures {
                if list || shown < 40 {
                    let text = match f.place {
                        Place::Sentence(s) => p.coach.text()[s].clone(),
                        Place::ShowLine(l) | Place::Narration { line: l, .. } => {
                            let sl = &p.coach.show_lines[l];
                            format!("[{}] {}", sl.label, sl.narration.join(" / "))
                        }
                    };
                    println!(
                        "{} ply {} {}: {} — {:?}: \"{}\" — {}\n    {text}",
                        g.id,
                        p.ply,
                        p.san,
                        f.kind.name(),
                        f.place,
                        f.claim,
                        f.detail
                    );
                }
                shown += 1;
            }
            total.add(report);
        }
    }
    if shown > 40 && !list {
        println!("... {} more (--list prints all)", shown - 40);
    }

    let pct = |n: usize, d: usize| {
        if d == 0 {
            0.0
        } else {
            100.0 * n as f64 / d as f64
        }
    };
    println!();
    println!(
        "{} games, {plies} plies, {} sentences, {} show-lines, {} narrations",
        games.len(),
        total.sentences,
        total.show_lines,
        total.narrations
    );
    println!("claims checked: {}", total.claims);
    println!(
        "sentences with a false claim: {} ({:.2}%)",
        total.false_sentences,
        pct(total.false_sentences, total.sentences)
    );
    println!(
        "narrations with a false claim: {} ({:.2}%)",
        total.false_narrations,
        pct(total.false_narrations, total.narrations)
    );
    println!(
        "claims past the end of their line (unchecked): {}",
        total.unchecked.len()
    );
    println!();
    println!("| kind | false claims | sentences with one | % of sentences |");
    println!("|---|---:|---:|---:|");
    let by_kind = total.by_kind();
    for k in Kind::ALL {
        let n = by_kind.get(&k).copied().unwrap_or(0);
        let s = sentences_by_kind.get(&k).copied().unwrap_or(0);
        println!(
            "| {} | {n} | {s} | {:.2}% |",
            k.name(),
            pct(s, total.sentences)
        );
    }
    if mutate {
        println!();
        println!("| mutation | tried | caught | rate |");
        println!("|---|---:|---:|---:|");
        for m in Mutation::ALL {
            let (tried, caught) = mutated.get(&m).copied().unwrap_or((0, 0));
            println!(
                "| {} | {tried} | {caught} | {:.1}% |",
                m.name(),
                pct(caught, tried)
            );
        }
    }
    Ok(())
}

fn load_json(path: &Path) -> Result<Option<Game>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if value.pointer("/result/plies").is_none() {
        return Ok(None);
    }
    let stored: Stored =
        serde_json::from_value(value).map_err(|e| format!("{}: {e}", path.display()))?;
    let id = stored.game_id.unwrap_or_else(|| {
        path.file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
    });
    Ok(Some(Game {
        id,
        player: stored.result.settings.and_then(|s| s.player_side),
        plies: stored.result.plies,
    }))
}

async fn review_pgn(path: &Path, depth: u32) -> Result<Game, String> {
    let pgn = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let game = parse_pgn(&pgn)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .into_iter()
        .next()
        .ok_or("no game")?
        .map_err(|e| e.to_string())?;
    let mut config = AppConfig::from_env().map_err(|e| e.to_string())?;
    let engine_path = config
        .engine_path
        .take()
        .ok_or("no engine: set STOCKFISH_PATH")?;
    let engine: Arc<dyn Evaluator> = Arc::new(
        EnginePool::start(engine_path, config.engine.clone())
            .await
            .map_err(|e| e.to_string())?,
    );
    let review = run_review(
        ReviewRequest {
            game,
            limit: SearchLimit::Depth { depth },
            multipv: 3,
            config: ReviewConfig::default(),
            player_side: None,
        },
        engine,
        load_openings(&config),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .map_err(|e| e.to_string())?;
    // The same shape as a stored review.
    let value = serde_json::json!({ "result": review });
    let stored: Stored = serde_json::from_value(value).map_err(|e| e.to_string())?;
    Ok(Game {
        id: path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
        player: None,
        plies: stored.result.plies,
    })
}
