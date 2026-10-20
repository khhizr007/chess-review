//! In-memory review jobs: queueing, progress, cancellation, retention.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use crate::analysis::model::ReviewResult;
use crate::analysis::{ProgressFn, ReviewError, ReviewRequest, run_review_with_model};
use crate::engine::Evaluator;
use crate::human::HumanModel;
use crate::openings::OpeningBook;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
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

#[derive(Debug, Clone, Copy, Default, Serialize, TS)]
#[ts(export)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
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
    result: Option<Arc<ReviewResult>>,
    finished_at: Option<Instant>,
}

#[derive(Clone)]
pub struct JobStore {
    jobs: Arc<Mutex<HashMap<Uuid, Job>>>,
    running: Arc<Semaphore>,
    max_stored: usize,
    /// The human-move model reviews use, when one is running.
    human: Option<Arc<dyn HumanModel>>,
}

impl JobStore {
    pub fn new(max_concurrent: usize, max_stored: usize) -> Self {
        Self {
            jobs: Arc::default(),
            running: Arc::new(Semaphore::new(max_concurrent.max(1))),
            max_stored: max_stored.max(1),
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
        {
            let mut jobs = self.jobs.lock().expect("job lock");
            Self::evict(&mut jobs, self.max_stored);
            jobs.insert(
                id,
                Job {
                    status: status.clone(),
                    cancel: cancel.clone(),
                    result: None,
                    finished_at: None,
                },
            );
        }

        let store = self.clone();
        tokio::spawn(async move {
            let permit = tokio::select! {
                () = cancel.cancelled() => None,
                permit = store.running.clone().acquire_owned() => permit.ok(),
            };
            let Some(_permit) = permit else {
                store.finish(id, Err(ReviewError::Cancelled));
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
            store.finish(id, result);
        });
        status
    }

    fn finish(&self, id: Uuid, result: Result<ReviewResult, ReviewError>) {
        self.update(id, |job| {
            job.finished_at = Some(Instant::now());
            match result {
                Ok(review) => {
                    job.status.state = JobState::Completed;
                    job.status.progress.done = job.status.progress.total;
                    job.result = Some(Arc::new(review));
                }
                Err(ReviewError::Cancelled) => job.status.state = JobState::Cancelled,
                Err(e) => {
                    job.status.state = JobState::Failed;
                    job.status.error = Some(e.to_string());
                }
            }
        });
    }

    /// Drops the oldest finished jobs once the store is full.
    fn evict(jobs: &mut HashMap<Uuid, Job>, max: usize) {
        while jobs.len() >= max {
            let oldest = jobs
                .iter()
                .filter_map(|(id, job)| job.finished_at.map(|t| (*id, t)))
                .min_by_key(|(_, t)| *t);
            match oldest {
                Some((id, _)) => {
                    jobs.remove(&id);
                }
                None => break,
            }
        }
    }

    pub fn status(&self, id: Uuid) -> Option<ReviewStatus> {
        self.jobs
            .lock()
            .expect("job lock")
            .get(&id)
            .map(|j| j.status.clone())
    }

    pub fn result(&self, id: Uuid) -> Option<(ReviewStatus, Option<Arc<ReviewResult>>)> {
        self.jobs
            .lock()
            .expect("job lock")
            .get(&id)
            .map(|j| (j.status.clone(), j.result.clone()))
    }

    /// Requests cancellation; returns the status, or `None` for unknown ids.
    pub fn cancel(&self, id: Uuid) -> Option<ReviewStatus> {
        let jobs = self.jobs.lock().expect("job lock");
        let job = jobs.get(&id)?;
        if !job.status.state.is_finished() {
            job.cancel.cancel();
        }
        Some(job.status.clone())
    }

    /// Cancels every unfinished job (shutdown).
    pub fn cancel_all(&self) {
        for job in self.jobs.lock().expect("job lock").values() {
            job.cancel.cancel();
        }
    }
}
