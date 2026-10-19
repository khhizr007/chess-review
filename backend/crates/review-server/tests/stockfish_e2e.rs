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
    engine_with_cache(None).await
}

/// Two engine processes; `cache_entries` overrides the analysis cache size.
async fn engine_with_cache(cache_entries: Option<usize>) -> Option<Arc<EnginePool>> {
    let mut config = AppConfig::from_env().ok()?;
    let path = config.engine_path.take()?;
    config.engine.pool_size = 2;
    config.engine.search_timeout = Duration::from_secs(30);
    if let Some(n) = cache_entries {
        config.engine.cache_entries = n;
    }
    Some(Arc::new(
        EnginePool::start(path, config.engine)
            .await
            .expect("engine starts"),
    ))
}

#[tokio::test]
async fn repeated_reviews_give_identical_results() {
    // With the analysis cache off every search really runs again, on engines whose
    // hash tables already hold earlier searches (same pool) or nothing (new pool).
    let Some(first_pool) = engine_with_cache(Some(0)).await else {
        eprintln!("skipping: no UCI engine (set STOCKFISH_PATH)");
        return;
    };
    let second_pool = engine_with_cache(Some(0)).await.expect("engine starts");
    let game = parse_pgn(&common::fixture("opera_game.pgn"))
        .unwrap()
        .remove(0)
        .unwrap();
    let mut runs = Vec::new();
    for pool in [&first_pool, &first_pool, &second_pool] {
        let evaluator: Arc<dyn Evaluator> = pool.clone();
        let r = run_review(
            ReviewRequest {
                game: game.clone(),
                limit: SearchLimit::Depth { depth: 12 },
                multipv: 3,
                config: ReviewConfig::default(),
            },
            evaluator,
            common::openings(),
            CancellationToken::new(),
            Arc::new(|_, _| {}),
        )
        .await
        .expect("review succeeds");
        let plies: Vec<_> = r
            .plies
            .iter()
            .map(|p| (p.classification, p.eval_after, p.ep_lost.to_bits()))
            .collect();
        runs.push(plies);
    }
    assert_eq!(runs[0], runs[1], "same pool, second review");
    assert_eq!(runs[0], runs[2], "fresh pool");
    first_pool.shutdown().await;
    second_pool.shutdown().await;
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
