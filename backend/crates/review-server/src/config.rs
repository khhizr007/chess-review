//! Server configuration from environment variables (see `docs/configuration.md`).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind: SocketAddr,
    /// Path to a UCI engine executable. `None` disables reviews (health reports it).
    pub engine_path: Option<PathBuf>,
    pub engine: EngineOptions,
    /// Directory with `*.tsv` opening files (eco, name, pgn).
    pub openings_dir: Option<PathBuf>,
    /// Built frontend to serve (optional; in development Vite serves it).
    pub static_dir: Option<PathBuf>,
    /// Allowed browser origin for CORS (development).
    pub cors_origin: Option<String>,
    /// Reviews that may run at the same time; others wait in the queue.
    pub max_concurrent_reviews: usize,
    /// Finished reviews kept in memory.
    pub max_stored_reviews: usize,
}

#[derive(Debug, Clone)]
pub struct EngineOptions {
    /// Engine processes in the pool (upper bound on parallel searches).
    pub pool_size: usize,
    pub threads: u32,
    pub hash_mb: u32,
    /// Hard limit for a single position search.
    pub search_timeout: Duration,
    /// Cached position analyses (LRU entries).
    pub cache_entries: usize,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{name} has an invalid value {value:?}: {reason}")]
    Invalid {
        name: &'static str,
        value: String,
        reason: String,
    },
}

fn parse_var<T: std::str::FromStr>(name: &'static str, default: T) -> Result<T, ConfigError>
where
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v.trim().parse::<T>().map_err(|e| ConfigError::Invalid {
            name,
            value: v,
            reason: e.to_string(),
        }),
        _ => Ok(default),
    }
}

fn optional_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Finds `stockfish` on `PATH` when `STOCKFISH_PATH` is not set.
fn find_on_path(binary: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2);
        let default_pool = (cpus / 2).clamp(1, 4);
        Ok(Self {
            bind: parse_var("BIND_ADDR", SocketAddr::from(([127, 0, 0, 1], 8080)))?,
            engine_path: optional_path("STOCKFISH_PATH").or_else(|| find_on_path("stockfish")),
            engine: EngineOptions {
                pool_size: parse_var("ENGINE_POOL_SIZE", default_pool)?.max(1),
                threads: parse_var("ENGINE_THREADS", 1u32)?.max(1),
                hash_mb: parse_var("ENGINE_HASH_MB", 64u32)?.max(1),
                search_timeout: Duration::from_secs(
                    parse_var("ENGINE_SEARCH_TIMEOUT_SECS", 60u64)?.max(1),
                ),
                cache_entries: parse_var("ANALYSIS_CACHE_ENTRIES", 20_000usize)?,
            },
            openings_dir: optional_path("OPENINGS_DIR").or_else(|| {
                let local = PathBuf::from("data/openings");
                local.is_dir().then_some(local)
            }),
            static_dir: optional_path("STATIC_DIR"),
            cors_origin: std::env::var("CORS_ORIGIN").ok().filter(|v| !v.is_empty()),
            max_concurrent_reviews: parse_var("MAX_CONCURRENT_REVIEWS", 2usize)?.max(1),
            max_stored_reviews: parse_var("MAX_STORED_REVIEWS", 100usize)?.max(1),
        })
    }
}
