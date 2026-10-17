use std::sync::Arc;

use review_server::api::AppState;
use review_server::config::AppConfig;
use review_server::engine::Evaluator;
use review_server::engine::pool::EnginePool;
use review_server::jobs::JobStore;
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

    let jobs = JobStore::new(config.max_concurrent_reviews, config.max_stored_reviews);
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
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down");
        })
        .await;

    jobs.cancel_all();
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
