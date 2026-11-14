use crate::domain::error::Severity;
use crate::domain::queue_item::QueueJob;
use crossterm::event::KeyEvent;

#[derive(Debug, Clone)]
pub enum Message {
    KeyPressed(KeyEvent),
    TerminalResized(u16, u16),
    PasteClipboardText(String),
    TriggerUrlExtraction(String),
    RawSubprocessLine(String),
    LogEvent {
        severity: Severity,
        module: String,
        summary: String,
    },
    JobProgressUpdated {
        job_id: String,
        percent_x10: u16,
        speed_bps: u64,
        eta_secs: Option<u64>,
    },
    JobCompleted {
        job_id: String,
    },
    JobFailed {
        job_id: String,
        error: String,
    },
}

/// Side-effect commands returned by `update()` to be executed by the async runtime coordinator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    ReadWaylandClipboard,
    SpawnExtractor {
        normalized_url: String,
        use_pbf03_cookies: bool,
    },
    EnqueueJobs(Vec<QueueJob>),
    PauseOrResumeJob {
        job_id: String,
    },
    CancelJobInTui {
        job_id: String,
    },
    ResumeRecoveredQueue,
    RestartRecoveredQueueFromScratch,
    ClearHistoryLedgerDbOnly,
    PurgeXdgCache,
    FlushQueueStateSnapshot,
    TransitionToCliProgressMode,
}
