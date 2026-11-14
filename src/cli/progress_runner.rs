use crate::domain::queue_item::{JobStatus, QueueJob};
use crate::storage::state_store::QueueStateStore;
use std::io::{self, Write};
use std::path::Path;

/// Handles the TUI-to-CLI Handoff when the user presses `Q` or `Ctrl+C` in TUI and confirms detach.
///
/// Key guarantees:
/// 1. Leaving TUI does NOT cancel active downloads; their progress is rendered as clean CLI lines.
/// 2. If the user presses `Ctrl+C` while in this CLI mode, `handle_cli_sigint` marks active jobs as
///    `JobStatus::InterruptedResumable` and flushes `$XDG_STATE_HOME/mvd/queue.json` while keeping
///    `~/Downloads/mvd/.staging/<job_id>/*.part` intact so reopening TUI can resume single or playlist jobs!
pub struct CliProgressHandoff;

impl CliProgressHandoff {
    pub fn format_cli_entry_line(idx_1_based: usize, job: &QueueJob) -> String {
        let pct = f64::from(job.progress_percent_x10) / 10.0;
        let speed_mib = (job.speed_bytes_per_sec as f64) / (1024.0 * 1024.0);
        let eta_str = job.eta_seconds.map_or_else(
            || "--:--".to_string(),
            |s| format!("{:02}:{:02}", s / 60, s % 60),
        );

        format!(
            "[*] #{idx_1_based:02} {:<9} [{pct:>5.1}%] {speed_mib:>5.2} MiB/s ETA {eta_str} | {} (.{})",
            job.status.badge(),
            job.metadata.title,
            job.target_container.extension()
        )
    }

    pub fn print_detach_banner(jobs: &[QueueJob]) -> io::Result<()> {
        let mut out = io::stdout();
        let active: Vec<&QueueJob> = jobs
            .iter()
            .filter(|j| {
                matches!(
                    j.status,
                    JobStatus::ActiveDownloading | JobStatus::MuxingPostProcess | JobStatus::Queued
                )
            })
            .collect();

        writeln!(
            out,
            "\n─[ MVD CLI PROGRESS MODE : {} JOB(S) RUNNING (Ctrl+C to stop & save .part resume state) ]─",
            active.len()
        )?;
        for (i, job) in active.iter().enumerate() {
            writeln!(out, "{}", Self::format_cli_entry_line(i + 1, job))?;
        }
        out.flush()
    }

    /// Invoked when `Ctrl+C` (`SIGINT`) is received in CLI mode:
    /// Marks in-flight jobs as `InterruptedResumable` and saves snapshot to `queue.json`.
    pub fn persist_resumable_state_on_cli_kill(
        state_file: &Path,
        jobs: &mut [QueueJob],
    ) -> Result<(), crate::domain::error::MvdError> {
        for job in jobs.iter_mut() {
            if matches!(
                job.status,
                JobStatus::ActiveDownloading | JobStatus::MuxingPostProcess
            ) {
                job.status = JobStatus::InterruptedResumable;
                job.speed_bytes_per_sec = 0;
            }
        }
        QueueStateStore::save_snapshot(state_file, jobs)
    }
}
