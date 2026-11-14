use crate::config::AppConfig;
use crate::domain::error::{Severity, TelemetryEvent};
use crate::domain::media::{
    AspectRatioKind, ContainerFormat, MediaArchetype, PlatformKind, StreamCandidate,
};
use crate::domain::metadata::EditableMetadata;
use crate::domain::playlist::PlaylistWorkspace;
use crate::domain::queue_item::{JobStatus, QueueJob};
use crate::domain::subtitle::SubtitleResolutionPlan;
use crate::engine::lrclib::LyricsDocument;
use crate::storage::ledger::HistoryRecord;

/// The 5 dedicated workspace pages of `mvd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePage {
    /// `[1] EXTRACTOR & PREVIEW`
    ExtractorPreview,
    /// `[2] QUEUE MANAGER` (Dedicated separate page)
    QueueManager,
    /// `[3] RAW TELEMETRY LOGS` (`yt-dlp` + `ffmpeg` raw stdout/stderr & structured ledger)
    RawTelemetryLogs,
    /// `[4] HISTORY LEDGER` (SQLite deduplication archive)
    HistoryLedger,
    /// `[5] SETTINGS & SECURITY` (XDG paths, `pbf03` Cookie Vault, EN/ID subs, cache purge)
    SettingsSecurity,
}

impl WorkspacePage {
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [
            Self::ExtractorPreview,
            Self::QueueManager,
            Self::RawTelemetryLogs,
            Self::HistoryLedger,
            Self::SettingsSecurity,
        ]
    }

    #[must_use]
    pub const fn tab_label(self) -> &'static str {
        match self {
            Self::ExtractorPreview => "1:EXTRACTOR",
            Self::QueueManager => "2:QUEUE",
            Self::RawTelemetryLogs => "3:RAW LOGS",
            Self::HistoryLedger => "4:HISTORY",
            Self::SettingsSecurity => "5:SETTINGS",
        }
    }
}

/// Modal input state (`NORMAL`, `INSERT`, `FILTER`, `MODAL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    InsertUrl,
    FilterSearch,
    ModalDialog,
}

impl InputMode {
    #[must_use]
    pub const fn token_badge(self) -> &'static str {
        match self {
            Self::Normal => "[NORMAL]",
            Self::InsertUrl => "[INSERT]",
            Self::FilterSearch => "[FILTER]",
            Self::ModalDialog => "[MODAL]",
        }
    }
}

/// Centered floating modals / dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModal {
    /// Triggered by `Q` or `Ctrl+C` in TUI: warns user that exiting TUI will detach active jobs to CLI progress mode.
    ConfirmDetachOrExit { active_job_count: usize },
    /// Triggered at startup if `$XDG_STATE_HOME/mvd/queue.json` contains unfinished jobs from a previous `Ctrl+C` CLI kill.
    ResumeUnfinishedQueuePrompt { unfinished_count: usize },
    /// Triggered by `p` or `n` ONLY when the user has unsaved custom metadata (`m`) or per-item overrides (`*`) on the current media.
    UnqueuedEditsGuard { pending_new_url: String },
    /// Triggered by `D` (`Quick Enqueue Clipboard`) to queue a new URL with a preset without replacing the active preview.
    QuickEnqueueDialog {
        url_input: String,
        preset_idx: usize,
    },
    /// Triggered by `m` on single media or on a highlighted playlist row (`item_index`).
    MetadataEditor {
        playlist_item_index: Option<usize>,
        draft: EditableMetadata,
        active_field_idx: usize,
    },
    /// Triggered by `f` on single media or on a highlighted playlist row (`item_index`) to override format/codec/subtitles.
    GranularStreamOverride {
        playlist_item_index: Option<usize>,
        selected_format_idx: usize,
    },
    /// Triggered by `X` on History Ledger (`Page [4]`): confirms non-destructive DB clear (`~/Downloads/mvd/` untouched).
    ConfirmClearHistoryDbOnly,
    /// Triggered by `e` on a failed job in Queue Manager.
    ErrorDiagnosticInspector {
        job_title: String,
        error_detail: String,
    },
}

/// Exit/Handoff state from the TUI event loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitTransition {
    StayInTui,
    /// User pressed `Q` / `Ctrl+C` and confirmed: leave TUI alternate screen and run live CLI progress output.
    DetachToCliProgress,
    /// Clean exit when zero active downloads exist.
    CleanExitImmediately,
}

/// Inspected media state on `Page [1] EXTRACTOR & PREVIEW` (strictly zero hardcoded dummy data).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PreviewInspectionState {
    Empty,
    FetchingSkeleton {
        target_url: String,
        platform: PlatformKind,
        expected_archetype: MediaArchetype,
    },
    LoadedSingle {
        media_id: String,
        canonical_url: String,
        platform: PlatformKind,
        archetype: MediaArchetype,
        aspect_ratio: AspectRatioKind,
        thumbnail_url: Option<String>,
        gallery_urls: Vec<String>,
        active_gallery_idx: usize,
        metadata: EditableMetadata,
        formats: Vec<StreamCandidate>,
        selected_format_idx: usize,
        target_container: ContainerFormat,
        subtitle_plan: SubtitleResolutionPlan,
        chapters: Vec<(String, u64)>,
        lyrics: Option<LyricsDocument>,
        lyrics_scroll_offset: u16,
        is_duplicate_in_ledger: bool,
    },
    LoadedCollection {
        platform: PlatformKind,
        archetype: MediaArchetype,
        workspace: PlaylistWorkspace,
        selected_row_idx: usize,
        active_row_lyrics: Option<LyricsDocument>,
        lyrics_scroll_offset: u16,
    },
}

/// Single source of truth for `mvd` (The Elm Architecture `Model`).
#[derive(Debug, Clone)]
pub struct AppModel {
    pub config: AppConfig,
    pub active_page: WorkspacePage,
    pub input_mode: InputMode,
    pub active_modal: Option<ActiveModal>,
    pub url_input_buffer: String,
    pub filter_query_buffer: String,
    pub inspection: PreviewInspectionState,
    pub has_unqueued_edits: bool,
    pub pbf03_cookie_enabled: bool,
    pub ephemeral_privacy_mode: bool,
    pub queue_jobs: Vec<QueueJob>,
    pub selected_queue_idx: usize,
    pub raw_subprocess_lines: Vec<String>,
    pub telemetry_events: Vec<TelemetryEvent>,
    pub history_records: Vec<HistoryRecord>,
    pub selected_history_idx: usize,
    pub cache_usage_bytes: u64,
    pub status_banner: Option<(Severity, String)>,
    pub dirty: bool,
    pub exit_transition: ExitTransition,
}

impl AppModel {
    #[must_use]
    pub fn new(config: AppConfig, recovered_jobs: Vec<QueueJob>) -> Self {
        let unfinished_count = recovered_jobs
            .iter()
            .filter(|j| j.status.is_unfinished_resumable())
            .count();

        let initial_modal = if unfinished_count > 0 {
            Some(ActiveModal::ResumeUnfinishedQueuePrompt { unfinished_count })
        } else {
            None
        };

        let initial_mode = if initial_modal.is_some() {
            InputMode::ModalDialog
        } else {
            InputMode::Normal
        };

        Self {
            config,
            active_page: WorkspacePage::ExtractorPreview,
            input_mode: initial_mode,
            active_modal: initial_modal,
            url_input_buffer: String::new(),
            filter_query_buffer: String::new(),
            inspection: PreviewInspectionState::Empty,
            has_unqueued_edits: false,
            pbf03_cookie_enabled: false,
            ephemeral_privacy_mode: false,
            queue_jobs: recovered_jobs,
            selected_queue_idx: 0,
            raw_subprocess_lines: Vec::new(),
            telemetry_events: Vec::new(),
            history_records: Vec::new(),
            selected_history_idx: 0,
            cache_usage_bytes: 0,
            status_banner: None,
            dirty: true,
            exit_transition: ExitTransition::StayInTui,
        }
    }

    #[must_use]
    pub fn active_download_count(&self) -> usize {
        self.queue_jobs
            .iter()
            .filter(|j| {
                matches!(
                    j.status,
                    JobStatus::ActiveDownloading | JobStatus::MuxingPostProcess
                )
            })
            .count()
    }
}
