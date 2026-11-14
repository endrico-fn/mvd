use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Utilitarian symbol-only severity tier (strictly zero emoji).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// `[i]` Informational telemetry (#3B82F6)
    Info,
    /// `[!]` Non-fatal warning or fallback notice (#F59E0B)
    Warning,
    /// `[x]` Operational, extractor, or security error (#EF4444)
    Error,
    /// `[+]` Successful completion (#28b877)
    Success,
}

impl Severity {
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Info => "[i]",
            Self::Warning => "[!]",
            Self::Error => "[x]",
            Self::Success => "[+]",
        }
    }
}

/// Structured telemetry log entry for Page [3] RAW TELEMETRY LOGS & Footer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub timestamp_epoch_ms: u64,
    pub severity: Severity,
    pub source_module: String,
    pub summary: String,
    pub raw_detail: Option<String>,
}

/// Domain-level error hierarchy for `mvd`.
#[derive(Debug, Error)]
pub enum MvdError {
    #[error("[x] SECURITY VIOLATION: Attempted access to forbidden Firefox profile '{attempted}'. Only sandbox profile 'pbf03' is permitted.")]
    ForbiddenProfile { attempted: String },

    #[error("[x] SECURITY VIOLATION: Sandbox profile 'pbf03' path validation failed: {reason}")]
    SandboxPathInvalid { reason: String },

    #[error("[x] RUNTIME VAULT ERROR: Ephemeral tmpfs directory '{path}' is unsafe or inaccessible: {reason}")]
    EphemeralVaultError { path: String, reason: String },

    #[error("[x] URL PARSE ERROR: Invalid or unsupported media URL '{input}': {reason}")]
    InvalidUrl { input: String, reason: String },

    #[error("[x] EXTRACTOR ERROR ({extractor}): {message}")]
    ExtractorFailed { extractor: String, message: String },

    #[error(
        "[!] AGE OR AUTH GATE: Media '{media_id}' requires sandbox cookie authentication (pbf03)."
    )]
    AuthRequired { media_id: String },

    #[error("[x]MUXING ERROR (ffmpeg exit {exit_code}): {stderr_tail}")]
    MuxingFailed { exit_code: i32, stderr_tail: String },

    #[error("[x] STORAGE LEDGER ERROR: {0}")]
    Storage(#[from] rusqlite::Error),

    #[error("[x] IO ERROR: {0}")]
    Io(#[from] std::io::Error),

    #[error("[x] JSON SERIALIZATION ERROR: {0}")]
    Json(#[from] serde_json::Error),
}
