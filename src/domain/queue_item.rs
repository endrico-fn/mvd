use crate::domain::media::{ContainerFormat, MediaArchetype, PlatformKind};
use crate::domain::metadata::EditableMetadata;
use crate::domain::subtitle::SubtitleResolutionPlan;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Lifecycle state of a queued download job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    ActiveDownloading,
    MuxingPostProcess,
    Paused,
    /// Interrupted when user killed CLI progress mode via `Ctrl+C` — `.part` files intact and resumable!
    InterruptedResumable,
    Completed,
    Failed,
    CancelledInTui,
}

impl JobStatus {
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Queued => "[QUEUED]",
            Self::ActiveDownloading => "[ACTIVE]",
            Self::MuxingPostProcess => "[MUXING]",
            Self::Paused => "[PAUSED]",
            Self::InterruptedResumable => "[RESUME]",
            Self::Completed => "[+ DONE]",
            Self::Failed => "[x FAIL]",
            Self::CancelledInTui => "[- CANC]",
        }
    }

    #[must_use]
    pub const fn is_unfinished_resumable(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::ActiveDownloading | Self::Paused | Self::InterruptedResumable
        )
    }
}

/// Persistent checkpoint saved to `$XDG_STATE_HOME/mvd/queue.json` so that if the user
/// detaches from TUI (`Q`) to CLI mode and presses `Ctrl+C`, reopening `mvd` can either
/// resume from the exact `.part` byte/fragment offset OR restart cleanly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueJob {
    pub job_id: String,
    pub parent_playlist_id: Option<String>,
    pub parent_playlist_title: Option<String>,
    pub playlist_index: Option<usize>,
    pub media_id: String,
    pub source_url: String,
    pub platform: PlatformKind,
    pub archetype: MediaArchetype,
    pub status: JobStatus,
    pub video_format_expr: Option<String>,
    pub audio_format_expr: Option<String>,
    pub target_container: ContainerFormat,
    pub metadata: EditableMetadata,
    pub subtitle_plan: SubtitleResolutionPlan,
    pub synced_lyrics_lrc: Option<String>,
    pub cover_art_url: Option<String>,
    pub use_pbf03_cookies: bool,
    pub ephemeral_privacy_mode: bool,
    pub staging_dir: PathBuf,
    pub final_output_path: PathBuf,
    pub progress_percent_x10: u16,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: Option<u64>,
    pub downloaded_bytes: u64,
    pub total_bytes_estimate: Option<u64>,
    pub last_error: Option<String>,
}

impl QueueJob {
    #[must_use]
    pub fn progress_fraction(&self) -> f64 {
        (f64::from(self.progress_percent_x10) / 1000.0).clamp(0.0, 1.0)
    }
}
