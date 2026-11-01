//! Bounded pool of UCI engine processes with an analysis cache.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lru::LruCache;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::uci::UciEngine;
use super::{AnalysisRequest, EngineError, Evaluator, PositionAnalysis};
use crate::config::EngineOptions;

pub struct EnginePool {
    path: PathBuf,
    options: EngineOptions,
    /// Limits concurrently running searches (and therefore processes).
    permits: Arc<Semaphore>,
    idle: Mutex<Vec<UciEngine>>,
    cache: Option<Mutex<LruCache<AnalysisRequest, PositionAnalysis>>>,
    name: Mutex<String>,
}

impl EnginePool {
    /// Starts one engine to validate the configuration and learn its name.
    pub async fn start(path: PathBuf, options: EngineOptions) -> Result<Self, EngineError> {
        let first = UciEngine::spawn(&path, options.threads, options.hash_mb).await?;
        let name = first.name().to_owned();
        Ok(Self {
            permits: Arc::new(Semaphore::new(options.pool_size)),
            cache: NonZeroUsize::new(options.cache_entries).map(|n| Mutex::new(LruCache::new(n))),
            idle: Mutex::new(vec![first]),
            name: Mutex::new(name),
            path,
            options,
        })
    }

    fn take_idle(&self) -> Option<UciEngine> {
        self.idle.lock().expect("engine pool lock").pop()
    }

    fn put_idle(&self, engine: UciEngine) {
        self.idle.lock().expect("engine pool lock").push(engine);
    }

    pub async fn shutdown(&self) {
        let engines: Vec<_> = std::mem::take(&mut *self.idle.lock().expect("engine pool lock"));
        for engine in engines {
            engine.shutdown().await;
        }
    }
}

#[async_trait]
impl Evaluator for EnginePool {
    async fn analyze(
        &self,
        request: &AnalysisRequest,
        cancel: &CancellationToken,
    ) -> Result<PositionAnalysis, EngineError> {
        if let Some(cache) = &self.cache
            && let Some(hit) = cache.lock().expect("cache lock").get(request)
        {
            return Ok(hit.clone());
        }

        let _permit = tokio::select! {
            () = cancel.cancelled() => return Err(EngineError::Cancelled),
            permit = self.permits.acquire() => permit.map_err(|_| EngineError::Crashed("pool closed".into()))?,
        };

        let mut engine = match self.take_idle() {
            Some(engine) => engine,
            None => {
                let engine =
                    UciEngine::spawn(&self.path, self.options.threads, self.options.hash_mb)
                        .await?;
                *self.name.lock().expect("name lock") = engine.name().to_owned();
                engine
            }
        };

        let result = engine
            .analyze(request, cancel, self.options.search_timeout)
            .await;
        if engine.is_healthy() {
            self.put_idle(engine);
        } else {
            tracing::warn!(error = ?result.as_ref().err(), "discarding unhealthy engine process");
        }

        if let (Ok(analysis), Some(cache)) = (&result, &self.cache) {
            cache
                .lock()
                .expect("cache lock")
                .put(request.clone(), analysis.clone());
        }
        result
    }

    fn name(&self) -> String {
        self.name.lock().expect("name lock").clone()
    }

    fn parallelism(&self) -> usize {
        self.options.pool_size
    }
}
