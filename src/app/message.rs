use crate::domain::error::Severity;
use crate::domain::media::{
    AspectRatioKind, ContainerFormat, MediaArchetype, PlatformKind, StreamCandidate,
};
use crate::domain::metadata::EditableMetadata;
use crate::domain::playlist::PlaylistWorkspace;
use crate::domain::queue_item::QueueJob;
use crate::domain::subtitle::SubtitleResolutionPlan;
use crate::engine::lrclib::LyricsDocument;
use crate::storage::ledger::HistoryRecord;
use crossterm::event::KeyEvent;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Message {
    KeyPressed(KeyEvent),
    TerminalResized(u16, u16),
    PasteClipboardText(String),
    TriggerUrlExtraction(String),
    ExtractionSucceededSingle {
        media_id: String,
        canonical_url: String,
        platform: PlatformKind,
        archetype: MediaArchetype,
        aspect_ratio: AspectRatioKind,
        thumbnail_url: Option<String>,
        metadata: EditableMetadata,
        formats: Vec<StreamCandidate>,
        target_container: ContainerFormat,
        subtitle_plan: SubtitleResolutionPlan,
        chapters: Vec<(String, u64)>,
        is_duplicate_in_ledger: bool,
    },
    ExtractionSucceededCollection {
        platform: PlatformKind,
        archetype: MediaArchetype,
        workspace: PlaylistWorkspace,
    },
    ExtractionFailed {
        target_url: String,
        error: String,
    },
    ThumbnailDecoded {
        url: String,
        image: image::DynamicImage,
    },
    LyricsFetched {
        media_id: String,
        lyrics: Option<LyricsDocument>,
    },
    PlaylistItemHydrated {
        media_id: String,
        aspect_ratio: AspectRatioKind,
        thumbnail_url: Option<String>,
        formats: Vec<StreamCandidate>,
        manual_sub_keys: Vec<String>,
        auto_sub_keys: Vec<String>,
    },
    HistoryRecordsRefreshed(Vec<HistoryRecord>),
    CacheUsageUpdated(u64),
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
    FetchCollectionRowPreview {
        media_id: String,
        canonical_url: String,
        title: String,
        artist: Option<String>,
        duration_secs: Option<u64>,
        thumbnail_url: Option<String>,
        is_audio: bool,
        needs_hydration: bool,
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
