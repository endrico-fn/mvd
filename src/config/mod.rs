pub mod xdg;

use crate::domain::media::ContainerFormat;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const ALLOWED_SANDBOX_PROFILE: &str = "pbf03";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub download_dir: PathBuf,
    pub max_concurrent_workers: usize,
    pub concurrent_fragments_per_job: u8,
    pub rate_limit_kbps: Option<u64>,
    pub default_video_container: ContainerFormat,
    pub default_audio_container: ContainerFormat,
    pub prefer_sub_en: bool,
    pub prefer_sub_id: bool,
    pub allow_auto_sub_fallback: bool,
    pub allowed_firefox_profile: String,
    pub max_cache_quota_mib: u64,
    pub sponsorblock_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            download_dir: home.join("Downloads").join("mvd"),
            max_concurrent_workers: 3,
            concurrent_fragments_per_job: 4,
            rate_limit_kbps: None,
            default_video_container: ContainerFormat::Mkv,
            default_audio_container: ContainerFormat::Opus,
            prefer_sub_en: true,
            prefer_sub_id: true,
            allow_auto_sub_fallback: true,
            allowed_firefox_profile: ALLOWED_SANDBOX_PROFILE.to_string(),
            max_cache_quota_mib: 256,
            sponsorblock_enabled: false,
        }
    }
}
