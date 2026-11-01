//! Review jobs: queueing, progress, cancellation, and hand-off of finished reviews to disk.
//!
//! Only unfinished jobs live in memory. A finished review is written to the [`ReviewStore`] and
//! removed from memory at once, so memory use follows the number of running and queued reviews,
//! not how many have ever finished. Reads of a finished review load it from disk.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use crate::analysis::model::ReviewResult;
use crate::analysis::{ProgressFn, ReviewError, ReviewRequest, run_review_with_model};
use crate::engine::Evaluator;
use crate::human::HumanModel;
use crate::openings::OpeningBook;
use crate::store::{ReviewStore, StoredReview, SweepReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            JobState::Completed | JobState::Failed | JobState::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReviewStatus {
    pub id: String,
    pub state: JobState,
    pub progress: Progress,
    pub error: Option<String>,
}

struct Job {
    status: ReviewStatus,
    cancel: CancellationToken,
    /// Set only when a finished job could not be written to disk: its status (never its
    /// result) then stays here until the sweep drops it.
    finished_at: Option<Instant>,
}

#[derive(Clone)]
pub struct JobStore {
    jobs: Arc<Mutex<HashMap<Uuid, Job>>>,
    running: Arc<Semaphore>,
    store: Arc<ReviewStore>,
    /// The human-move model reviews use, when one is running.
    human: Option<Arc<dyn HumanModel>>,
}

impl JobStore {
    pub fn new(max_concurrent: usize, store: ReviewStore) -> Self {
        Self {
            jobs: Arc::default(),
            running: Arc::new(Semaphore::new(max_concurrent.max(1))),
            store: Arc::new(store),
            human: None,
        }
    }

    /// Reviews use `model` for the coach's difficulty wording.
    pub fn with_human_model(mut self, model: Arc<dyn HumanModel>) -> Self {
        self.human = Some(model);
        self
    }

    fn update(&self, id: Uuid, f: impl FnOnce(&mut Job)) {
        if let Some(job) = self.jobs.lock().expect("job lock").get_mut(&id) {
            f(job);
        }
    }

    /// Queues a review and starts it when a slot is free.
    pub fn submit(
        &self,
        request: ReviewRequest,
        evaluator: Arc<dyn Evaluator>,
        openings: Arc<OpeningBook>,
    ) -> ReviewStatus {
        let id = Uuid::new_v4();
        let cancel = CancellationToken::new();
        let status = ReviewStatus {
            id: id.to_string(),
            state: JobState::Queued,
            progress: Progress::default(),
            error: None,
        };
        self.jobs.lock().expect("job lock").insert(
            id,
            Job {
                status: status.clone(),
                cancel: cancel.clone(),
                finished_at: None,
            },
        );

        let store = self.clone();
        tokio::spawn(async move {
            let permit = tokio::select! {
                () = cancel.cancelled() => None,
                permit = store.running.clone().acquire_owned() => permit.ok(),
            };
            let Some(_permit) = permit else {
                store.finish(id, Err(ReviewError::Cancelled)).await;
                return;
            };
            store.update(id, |job| job.status.state = JobState::Running);
            let progress_store = store.clone();
            let progress: ProgressFn = Arc::new(move |done, total| {
                progress_store.update(id, |job| job.status.progress = Progress { done, total });
            });
            let started = Instant::now();
            let human = store.human.clone();
            let result =
                run_review_with_model(request, evaluator, human, openings, cancel, progress).await;
            tracing::info!(review = %id, ok = result.is_ok(), elapsed_ms = started.elapsed().as_millis() as u64, "review finished");
            store.finish(id, result).await;
        });
        status
    }

    /// Writes a finished review to disk, then drops it from memory.
    ///
    /// The file is written before the job leaves the map, so a reader always finds the review in
    /// one place or the other. Until then readers see the job as running.
    async fn finish(&self, id: Uuid, result: Result<ReviewResult, ReviewError>) {
        let Some(mut status) = self.status_in_memory(id) else {
            return;
        };
        let review = match result {
            Ok(review) => {
                status.state = JobState::Completed;
                status.progress.done = status.progress.total;
                Some(review)
            }
            Err(ReviewError::Cancelled) => {
                status.state = JobState::Cancelled;
                None
            }
            Err(e) => {
                status.state = JobState::Failed;
                status.error = Some(e.to_string());
                None
            }
        };

        let store = Arc::clone(&self.store);
        let saved_status = status.clone();
        let saved =
            tokio::task::spawn_blocking(move || store.save(id, &saved_status, review.as_ref()))
                .await
                .unwrap_or_else(|e| Err(io::Error::other(e)));

        let mut jobs = self.jobs.lock().expect("job lock");
        match saved {
            Ok(()) => {
                jobs.remove(&id);
            }
            Err(e) => {
                // Keep the small status only: holding the result would bring back unbounded
                // memory use whenever the disk is full or unwritable.
                tracing::error!(review = %id, "cannot save the finished review: {e}");
                if let Some(job) = jobs.get_mut(&id) {
                    job.finished_at = Some(Instant::now());
                    job.status = ReviewStatus {
                        state: JobState::Failed,
                        error: Some(format!("the review could not be saved: {e}")),
                        ..status
                    };
                }
            }
        }
    }

    fn status_in_memory(&self, id: Uuid) -> Option<ReviewStatus> {
        self.jobs
            .lock()
            .expect("job lock")
            .get(&id)
            .map(|j| j.status.clone())
    }

    async fn load(&self, id: Uuid) -> io::Result<Option<StoredReview>> {
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || store.load(id))
            .await
            .unwrap_or_else(|e| Err(io::Error::other(e)))
    }

    /// The review's status; `None` for unknown or expired ids.
    pub async fn status(&self, id: Uuid) -> io::Result<Option<ReviewStatus>> {
        if let Some(status) = self.status_in_memory(id) {
            return Ok(Some(status));
        }
        Ok(self.load(id).await?.map(|stored| stored.status))
    }

    /// The review's status and, once it has completed, its result.
    pub async fn result(
        &self,
        id: Uuid,
    ) -> io::Result<Option<(ReviewStatus, Option<ReviewResult>)>> {
        if let Some(status) = self.status_in_memory(id) {
            return Ok(Some((status, None)));
        }
        Ok(self
            .load(id)
            .await?
            .map(|stored| (stored.status, stored.result)))
    }

    /// Requests cancellation; returns the status, or `None` for unknown ids.
    pub async fn cancel(&self, id: Uuid) -> io::Result<Option<ReviewStatus>> {
        {
            let jobs = self.jobs.lock().expect("job lock");
            if let Some(job) = jobs.get(&id) {
                if !job.status.state.is_finished() {
                    job.cancel.cancel();
                }
                return Ok(Some(job.status.clone()));
            }
        }
        // Finished reviews are on disk and can no longer be cancelled.
        self.status(id).await
    }

    /// Cancels every unfinished job (shutdown).
    pub fn cancel_all(&self) {
        for job in self.jobs.lock().expect("job lock").values() {
            job.cancel.cancel();
        }
    }

    /// Unfinished jobs held in memory.
    pub fn in_memory(&self) -> usize {
        self.jobs.lock().expect("job lock").len()
    }

    /// Deletes expired and surplus reviews from disk, and drops the statuses of finished jobs
    /// that could not be saved once they are older than the time to live.
    pub async fn sweep(&self) -> io::Result<SweepReport> {
        let ttl = self.store.ttl();
        self.jobs
            .lock()
            .expect("job lock")
            .retain(|_, job| job.finished_at.is_none_or(|t| t.elapsed() < ttl));
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || store.sweep())
            .await
            .unwrap_or_else(|e| Err(io::Error::other(e)))
    }

    /// Runs [`sweep`](Self::sweep) now and then every `every`, until `stop` is cancelled.
    pub fn spawn_sweeper(
        &self,
        every: Duration,
        stop: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let jobs = self.clone();
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(every.max(Duration::from_secs(1)));
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    () = stop.cancelled() => break,
                    _ = ticks.tick() => {}
                }
                match jobs.sweep().await {
                    Ok(report) if report.expired + report.over_cap > 0 => tracing::info!(
                        expired = report.expired,
                        over_cap = report.over_cap,
                        kept = report.kept,
                        "deleted stored reviews"
                    ),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("cannot sweep stored reviews: {e}"),
                }
            }
        })
    }
}
