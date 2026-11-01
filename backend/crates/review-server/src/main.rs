use std::sync::Arc;

use review_server::api::AppState;
use review_server::config::AppConfig;
use review_server::engine::Evaluator;
use review_server::engine::pool::EnginePool;
use review_server::human::{HumanModel, MaiaSidecar};
use review_server::jobs::JobStore;
use review_server::store::ReviewStore;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=warn")),
        )
        .init();

    let config = match AppConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("configuration error: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let openings = review_server::load_openings(&config);

    let (pool, engine_error) = match &config.engine_path {
        Some(path) => match EnginePool::start(path.clone(), config.engine.clone()).await {
            Ok(pool) => {
                tracing::info!(engine = %pool.name(), pool_size = config.engine.pool_size, "engine ready");
                (Some(Arc::new(pool)), None)
            }
            Err(e) => {
                tracing::error!(path = %path.display(), "engine failed to start: {e}");
                (
                    None,
                    Some(format!("engine at {} failed to start: {e}", path.display())),
                )
            }
        },
        None => {
            tracing::warn!("no engine found: set STOCKFISH_PATH to enable reviews");
            (
                None,
                Some("no engine configured: set STOCKFISH_PATH".to_owned()),
            )
        }
    };

    let store = match ReviewStore::open(
        &config.review_store_dir,
        config.review_ttl,
        config.max_stored_reviews,
    ) {
        Ok(store) => store,
        Err(e) => {
            tracing::error!(dir = %config.review_store_dir.display(), "cannot open the review store: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    tracing::info!(
        dir = %store.dir().display(),
        ttl_secs = store.ttl().as_secs(),
        max = config.max_stored_reviews,
        "review store ready"
    );
    // Expired reviews are never served (reads check their age); the sweep reclaims the disk.
    let sweep_every = (config.review_ttl / 24).clamp(
        std::time::Duration::from_secs(60),
        std::time::Duration::from_secs(600),
    );
    let mut jobs = JobStore::new(config.max_concurrent_reviews, store);
    let stop_sweeper = CancellationToken::new();
    let sweeper = jobs.spawn_sweeper(sweep_every, stop_sweeper.clone());
    if let Some(command) = &config.human_model_command {
        match MaiaSidecar::start(command).await {
            Ok(model) => {
                tracing::info!(model = %model.name(), "human-move model ready");
                jobs = jobs.with_human_model(Arc::new(model));
            }
            Err(e) => tracing::warn!("human-move model unavailable, reviews run without it: {e}"),
        }
    }
    let state = AppState {
        evaluator: pool.clone().map(|p| p as Arc<dyn Evaluator>),
        engine_error,
        openings,
        jobs: jobs.clone(),
    };
    let app = review_server::app(state, &config);

    let listener = match tokio::net::TcpListener::bind(config.bind).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(bind = %config.bind, "cannot listen: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    tracing::info!("listening on http://{}", config.bind);

    let served = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            shutdown_signal().await;
            tracing::info!("shutting down");
        })
        .await;

    jobs.cancel_all();
    stop_sweeper.cancel();
    let _ = sweeper.await;
    if let Some(pool) = pool {
        pool.shutdown().await;
    }
    match served {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("server error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Resolves on Ctrl-C, or on SIGTERM (what `docker stop` and most hosts send) on Unix.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::warn!("cannot listen for SIGTERM: {e}");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
