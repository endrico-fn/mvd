use crate::domain::error::MvdError;
use crate::domain::queue_item::{JobStatus, QueueJob};
use std::fs;
use std::path::Path;

/// Persists the active download queue to `$XDG_STATE_HOME/mvd/queue.json`.
/// Enables seamless recovery after TUI-to-CLI detach (`Q`) + CLI `Ctrl+C` termination!
pub struct QueueStateStore;

impl QueueStateStore {
    pub fn save_snapshot(state_file: &Path, jobs: &[QueueJob]) -> Result<(), MvdError> {
        if let Some(parent) = state_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(jobs)?;
        fs::write(state_file, json)?;
        Ok(())
    }

    pub fn load_snapshot(state_file: &Path) -> Result<Vec<QueueJob>, MvdError> {
        if !state_file.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(state_file)?;
        let mut jobs: Vec<QueueJob> = serde_json::from_str(&content)?;
        // Any job that was mid-download when CLI was killed via Ctrl+C becomes InterruptedResumable
        for job in &mut jobs {
            if job.status == JobStatus::ActiveDownloading
                || job.status == JobStatus::MuxingPostProcess
            {
                job.status = JobStatus::InterruptedResumable;
                job.speed_bytes_per_sec = 0;
            }
        }
        Ok(jobs)
    }
}
