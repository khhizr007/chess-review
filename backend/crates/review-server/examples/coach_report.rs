//! Reviews a PGN game with a real engine and prints what the coach says about
//! each move it explains (development tooling for the coach).
//!
//! ```text
//! STOCKFISH_PATH=/path/to/stockfish \
//!   cargo run --release -p review-server --example coach_report -- <game.pgn> [depth] [threat depth]
//! ```

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use review_core::ReviewConfig;
use review_server::analysis::{ReviewRequest, run_review};
use review_server::coach::explains;
use review_server::config::AppConfig;
use review_server::engine::pool::EnginePool;
use review_server::engine::{Evaluator, SearchLimit};
use review_server::load_openings;
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("coach_report: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let number = |s: &String| s.parse::<u32>().map_err(|_| format!("bad depth {s}"));
    let mut review_config = ReviewConfig::default();
    let (pgn_path, depth) = match args.as_slice() {
        [p] => (p, 18),
        [p, d] => (p, number(d)?),
        [p, d, t] => {
            review_config.coach.threat_search_depth = number(t)?;
            (p, number(d)?)
        }
        _ => return Err("usage: coach_report <game.pgn> [depth] [threat depth]".to_owned()),
    };
    let pgn = std::fs::read_to_string(pgn_path).map_err(|e| format!("{pgn_path}: {e}"))?;
    let game = parse_pgn(&pgn)
        .map_err(|e| format!("{pgn_path}: {e}"))?
        .into_iter()
        .next()
        .ok_or("no game")?
        .map_err(|e| e.to_string())?;
    let mut config = AppConfig::from_env().map_err(|e| e.to_string())?;
    let path = config
        .engine_path
        .take()
        .ok_or("no engine: set STOCKFISH_PATH")?;
    let engine: Arc<dyn Evaluator> = Arc::new(
        EnginePool::start(path, config.engine.clone())
            .await
            .map_err(|e| e.to_string())?,
    );
    let started = Instant::now();
    let review = run_review(
        ReviewRequest {
            game,
            limit: SearchLimit::Depth { depth },
            multipv: 3,
            config: review_config,
        },
        engine,
        load_openings(&config),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .map_err(|e| e.to_string())?;
    println!(
        "Review took {:.1} s (depth {depth}, threat search depth {}).",
        started.elapsed().as_secs_f64(),
        review.config.coach.threat_search_depth
    );

    for p in review.plies.iter().filter(|p| explains(p.classification)) {
        let dots = if p.mover == review_core::Side::White {
            "."
        } else {
            "..."
        };
        println!(
            "{}{dots}{} ({:?}, best {})",
            p.move_number,
            p.san,
            p.classification.expect("explained moves are classified"),
            p.best_move.as_ref().map_or("-", |m| m.san.as_str())
        );
        if p.coach.sentences.is_empty() {
            println!("    (no coach sentence)");
            for l in &p.lines {
                let moves: Vec<&str> = l.moves.iter().map(|m| m.san.as_str()).collect();
                println!("    line {} {:?}: {}", l.rank, l.eval, moves.join(" "));
            }
        }
        for s in p.coach.text() {
            println!("    {s}");
        }
        if let Some(e) = p.coach.main_event() {
            println!(
                "    main: {:?} {:?} on {:?}, gain {:?}, mate {:?}, geometry {:?}",
                e.chain, e.tag, e.square, e.gain_cp, e.mate_in, e.geometry
            );
        }
        for line in &p.coach.show_lines {
            println!("    [{}] from {}", line.label, line.start_fen);
            for (m, n) in line.moves.iter().zip(&line.narration) {
                println!("        {:8} {n}", m.san);
            }
        }
    }
    Ok(())
}
