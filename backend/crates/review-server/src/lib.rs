//! Chess game-review server.

pub mod analysis;
pub mod api;
pub mod board;
pub mod config;
pub mod engine;
pub mod jobs;
pub mod openings;
pub mod pgn;

use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderValue, Method, header};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use api::AppState;
use config::AppConfig;

/// Builds the full application router (API, optional static frontend, CORS, tracing).
pub fn app(state: AppState, config: &AppConfig) -> Router {
    let mut router = api::router(state);
    if let Some(dir) = &config.static_dir {
        let index = dir.join("index.html");
        router = router.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }
    if let Some(origin) = config
        .cors_origin
        .as_deref()
        .and_then(|o| HeaderValue::from_str(o).ok())
    {
        router = router.layer(
            CorsLayer::new()
                .allow_origin(origin)
                .allow_methods([Method::GET, Method::POST, Method::DELETE])
                .allow_headers([header::CONTENT_TYPE]),
        );
    }
    router.layer(TraceLayer::new_for_http())
}

/// Loads the opening book, logging (not failing) when it is unavailable.
pub fn load_openings(config: &AppConfig) -> Arc<openings::OpeningBook> {
    let Some(dir) = &config.openings_dir else {
        tracing::warn!(
            "OPENINGS_DIR not set and ./data/openings missing: Book classification disabled"
        );
        return Arc::new(openings::OpeningBook::empty());
    };
    match openings::OpeningBook::load_dir(dir) {
        Ok((book, skipped)) => {
            tracing::info!(positions = book.len(), skipped, dir = %dir.display(), "opening book loaded");
            Arc::new(book)
        }
        Err(e) => {
            tracing::warn!(error = %e, dir = %dir.display(), "could not load opening book");
            Arc::new(openings::OpeningBook::empty())
        }
    }
}
