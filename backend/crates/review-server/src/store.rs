//! Finished reviews on disk: one JSON file per review, kept for a fixed time after it finished.
//!
//! The server holds only unfinished jobs in memory. When a review finishes, its status and
//! result are written here and dropped from memory; every later read loads the file again (the
//! operating system's page cache keeps recently read files cheap, and that memory can be
//! reclaimed under pressure).
//!
//! Expiry works like Redis' two-part scheme: a read checks the file's age and deletes it when it
//! is older than the time to live (so nothing is served past it), and a periodic [`sweep`]
//! deletes files nobody reads again. The sweep also deletes the oldest files beyond a count cap,
//! which bounds disk use when many reviews finish within one time to live.
//!
//! Files are written to a temporary name and renamed into place, so a reader never sees a
//! half-written review and a crash leaves at most a stray temporary file, removed on start.
//! Only names of the form `<uuid>.json` (and their temporary files) are ever read or deleted, so
//! pointing the store at a directory that holds other files does not touch them.
//!
//! [`sweep`]: ReviewStore::sweep

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::analysis::model::ReviewResult;
use crate::jobs::ReviewStatus;

const EXTENSION: &str = "json";
const TEMP_SUFFIX: &str = ".json.tmp";

/// A finished review as stored on disk.
#[derive(Debug, Deserialize)]
pub struct StoredReview {
    pub status: ReviewStatus,
    /// Set for completed reviews; failed and cancelled ones keep only their status.
    pub result: Option<ReviewResult>,
}

#[derive(Serialize)]
struct StoredReviewRef<'a> {
    status: &'a ReviewStatus,
    result: Option<&'a ReviewResult>,
}

/// What a [`ReviewStore::sweep`] removed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepReport {
    /// Reviews older than the time to live.
    pub expired: usize,
    /// The oldest reviews beyond the count cap.
    pub over_cap: usize,
    /// Reviews still stored.
    pub kept: usize,
}

#[derive(Debug)]
pub struct ReviewStore {
    dir: PathBuf,
    ttl: Duration,
    max_reviews: usize,
}

impl ReviewStore {
    /// Opens (creating it when missing) the store in `dir` and removes temporary files left by an
    /// interrupted write.
    pub fn open(dir: impl Into<PathBuf>, ttl: Duration, max_reviews: usize) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let store = Self {
            dir,
            ttl,
            max_reviews: max_reviews.max(1),
        };
        for entry in fs::read_dir(&store.dir)? {
            let path = entry?.path();
            if is_temp_file(&path) {
                remove_quietly(&path);
            }
        }
        Ok(store)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    fn path(&self, id: Uuid) -> PathBuf {
        self.dir.join(format!("{}.{EXTENSION}", id.hyphenated()))
    }

    /// Writes a finished review. Its time to live starts now.
    pub fn save(
        &self,
        id: Uuid,
        status: &ReviewStatus,
        result: Option<&ReviewResult>,
    ) -> io::Result<()> {
        let bytes = serde_json::to_vec(&StoredReviewRef { status, result })?;
        let path = self.path(id);
        let temp = self.dir.join(format!("{}{TEMP_SUFFIX}", id.hyphenated()));
        let written = (|| {
            let mut file = fs::File::create(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, &path)
        })();
        if written.is_err() {
            remove_quietly(&temp);
        }
        written
    }

    /// Reads a finished review; `None` when there is none or it has expired (and is then deleted).
    pub fn load(&self, id: Uuid) -> io::Result<Option<StoredReview>> {
        let path = self.path(id);
        let modified = match fs::metadata(&path) {
            Ok(meta) => meta.modified()?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if self.is_expired(modified, SystemTime::now()) {
            remove_quietly(&path);
            return Ok(None);
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            // Swept between the two calls.
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        serde_json::from_slice(&bytes).map(Some).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        })
    }

    fn is_expired(&self, modified: SystemTime, now: SystemTime) -> bool {
        // A modification time in the future (clock change) counts as age 0.
        now.duration_since(modified).unwrap_or_default() >= self.ttl
    }

    /// Deletes expired reviews, then the oldest ones beyond the count cap.
    pub fn sweep(&self) -> io::Result<SweepReport> {
        self.sweep_at(SystemTime::now())
    }

    fn sweep_at(&self, now: SystemTime) -> io::Result<SweepReport> {
        let mut report = SweepReport::default();
        let mut kept = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !is_review_file(&path) {
                continue;
            }
            // A file deleted meanwhile has no metadata; skip it.
            let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if self.is_expired(modified, now) {
                if remove_quietly(&path) {
                    report.expired += 1;
                }
            } else {
                kept.push((modified, path));
            }
        }
        if kept.len() > self.max_reviews {
            kept.sort_by_key(|(modified, _)| *modified);
            let excess = kept.len() - self.max_reviews;
            for (_, path) in kept.drain(..excess) {
                if remove_quietly(&path) {
                    report.over_cap += 1;
                }
            }
        }
        report.kept = kept.len();
        Ok(report)
    }
}

fn is_review_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == EXTENSION)
        && path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| Uuid::parse_str(s).is_ok())
}

fn is_temp_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(TEMP_SUFFIX))
        .is_some_and(|stem| Uuid::parse_str(stem).is_ok())
}

/// Deletes a file, logging failures other than "already gone". Returns whether it was deleted.
fn remove_quietly(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => {
            tracing::warn!(path = %path.display(), "cannot delete stored review: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{JobState, Progress};

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("review-store-test-{}", Uuid::new_v4()))
    }

    fn status(id: Uuid, state: JobState) -> ReviewStatus {
        ReviewStatus {
            id: id.to_string(),
            state,
            progress: Progress { done: 3, total: 3 },
            error: None,
        }
    }

    #[test]
    fn saves_loads_and_expires() {
        let dir = temp_dir();
        let store = ReviewStore::open(&dir, Duration::from_secs(60), 10).unwrap();
        let id = Uuid::new_v4();
        assert!(store.load(id).unwrap().is_none());

        store
            .save(id, &status(id, JobState::Cancelled), None)
            .unwrap();
        let loaded = store.load(id).unwrap().unwrap();
        assert_eq!(loaded.status.state, JobState::Cancelled);
        assert!(loaded.result.is_none());

        // Still there a second before the time to live, gone at it.
        let modified = fs::metadata(store.path(id)).unwrap().modified().unwrap();
        let report = store.sweep_at(modified + Duration::from_secs(59)).unwrap();
        assert_eq!((report.expired, report.kept), (0, 1));
        let report = store.sweep_at(modified + Duration::from_secs(60)).unwrap();
        assert_eq!((report.expired, report.kept), (1, 0));
        assert!(store.load(id).unwrap().is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_read_does_not_serve_an_expired_review() {
        let dir = temp_dir();
        let store = ReviewStore::open(&dir, Duration::ZERO, 10).unwrap();
        let id = Uuid::new_v4();
        store.save(id, &status(id, JobState::Failed), None).unwrap();
        assert!(store.load(id).unwrap().is_none());
        assert!(
            !store.path(id).exists(),
            "the expired file is deleted on read"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_cap_drops_the_oldest_and_other_files_are_left_alone() {
        let dir = temp_dir();
        let store = ReviewStore::open(&dir, Duration::from_secs(3600), 2).unwrap();
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        for id in &ids {
            store
                .save(*id, &status(*id, JobState::Cancelled), None)
                .unwrap();
            // Distinct modification times on filesystems with coarse timestamps.
            std::thread::sleep(Duration::from_millis(20));
        }
        fs::write(dir.join("notes.json"), "{}").unwrap();
        fs::write(dir.join("readme.txt"), "keep").unwrap();

        let report = store.sweep().unwrap();
        assert_eq!((report.over_cap, report.kept), (1, 2));
        assert!(store.load(ids[0]).unwrap().is_none());
        assert!(store.load(ids[2]).unwrap().is_some());
        assert!(dir.join("notes.json").exists());
        assert!(dir.join("readme.txt").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_removes_interrupted_writes() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let stray = dir.join(format!("{}{TEMP_SUFFIX}", Uuid::new_v4()));
        fs::write(&stray, "half").unwrap();
        let _store = ReviewStore::open(&dir, Duration::from_secs(60), 10).unwrap();
        assert!(!stray.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_corrupt_file_is_an_error() {
        let dir = temp_dir();
        let store = ReviewStore::open(&dir, Duration::from_secs(60), 10).unwrap();
        let id = Uuid::new_v4();
        fs::write(store.path(id), "not json").unwrap();
        let err = store.load(id).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        fs::remove_dir_all(&dir).unwrap();
    }
}
