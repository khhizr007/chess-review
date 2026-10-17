//! Full review with a real UCI engine. Skipped (passes trivially) when no
//! engine is found via `STOCKFISH_PATH` or `PATH`.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use review_core::{Classification, ReviewConfig};
use review_server::analysis::{ReviewRequest, run_review};
use review_server::config::AppConfig;
use review_server::engine::pool::EnginePool;
use review_server::engine::{Evaluator, SearchLimit};
use review_server::pgn::parse_pgn;
use tokio_util::sync::CancellationToken;

async fn engine() -> Option<Arc<EnginePool>> {
    let mut config = AppConfig::from_env().ok()?;
    let path = config.engine_path.take()?;
    config.engine.pool_size = 2;
    config.engine.search_timeout = Duration::from_secs(30);
    Some(Arc::new(
        EnginePool::start(path, config.engine)
            .await
            .expect("engine starts"),
    ))
}

#[tokio::test]
async fn opera_game_with_stockfish() {
    let Some(pool) = engine().await else {
        eprintln!("skipping: no UCI engine (set STOCKFISH_PATH)");
        return;
    };
    let game = parse_pgn(&common::fixture("opera_game.pgn"))
        .unwrap()
        .remove(0)
        .unwrap();
    let request = ReviewRequest {
        game,
        limit: SearchLimit::Depth { depth: 14 },
        multipv: 3,
        config: ReviewConfig::default(),
    };
    let started = Instant::now();
    let evaluator: Arc<dyn Evaluator> = pool.clone();
    let r = run_review(
        request,
        evaluator,
        common::openings(),
        CancellationToken::new(),
        Arc::new(|_, _| {}),
    )
    .await
    .expect("review succeeds");
    eprintln!(
        "engine: {} | {} plies in {:?}",
        r.engine,
        r.plies.len(),
        started.elapsed()
    );
    for p in &r.plies {
        eprintln!(
            "{:>2} {:<7} {:<12?} lost {:.3} acc {:>6.1} best {:<6} {}",
            p.ply,
            p.san,
            p.classification,
            p.ep_lost,
            p.accuracy.unwrap_or(f64::NAN),
            p.best_move.as_ref().map(|m| m.san.as_str()).unwrap_or("-"),
            if p.is_key_moment { "key" } else { "" }
        );
    }
    eprintln!(
        "accuracy white {:?} black {:?}",
        r.white.accuracy.overall, r.black.accuracy.overall
    );

    let last = r.plies.last().unwrap();
    assert_eq!(last.san, "Rd8#");
    assert!(last.flags.checkmate);
    assert_eq!(last.classification, Some(Classification::Best));
    assert!(r.plies[0].is_book, "1.e4 is book");
    assert!(r.white.accuracy.overall.unwrap() > r.black.accuracy.overall.unwrap());
    assert_eq!(r.white.blunders, 0, "Morphy's moves are not blunders");
    pool.shutdown().await;
}
