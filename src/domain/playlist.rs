use crate::domain::media::{AspectRatioKind, ContainerFormat, StreamCandidate};
use crate::domain::metadata::EditableMetadata;
use crate::domain::subtitle::SubtitleResolutionPlan;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Global ceiling & fallback policy applied across all items in a Playlist or Album
/// unless an individual item has an explicit `ItemOverride` (`[*]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalPlaylistPolicy {
    pub subfolder_name: String,
    pub max_height_ceiling: Option<u32>,
    pub preferred_vcodec: Option<String>,
    pub preferred_acodec: Option<String>,
    pub container: ContainerFormat,
    pub want_sub_en: bool,
    pub want_sub_id: bool,
    pub allow_auto_sub_fallback: bool,
}

impl GlobalPlaylistPolicy {
    #[must_use]
    pub fn default_video_playlist(title: &str) -> Self {
        Self {
            subfolder_name: sanitize_folder_name(title),
            max_height_ceiling: Some(1080),
            preferred_vcodec: Some("av01".to_string()),
            preferred_acodec: Some("opus".to_string()),
            container: ContainerFormat::Mkv,
            want_sub_en: true,
            want_sub_id: true,
            allow_auto_sub_fallback: true,
        }
    }

    #[must_use]
    pub fn default_audio_album(title: &str) -> Self {
        Self {
            subfolder_name: sanitize_folder_name(title),
            max_height_ceiling: None,
            preferred_vcodec: None,
            preferred_acodec: Some("opus".to_string()),
            container: ContainerFormat::Opus,
            want_sub_en: false,
            want_sub_id: false,
            allow_auto_sub_fallback: false,
        }
    }
}

/// Granular per-item override (`[*]`) inside a heterogeneous playlist or album.
/// Allows the user to customize resolution, codec, container, subtitles, or metadata for
/// any individual track/episode (`f`, `s`, or `m` keys on a playlist row).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistItemOverride {
    pub custom_video_format_id: Option<String>,
    pub custom_audio_format_id: Option<String>,
    pub custom_container: Option<ContainerFormat>,
    pub custom_sub_en: Option<bool>,
    pub custom_sub_id: Option<bool>,
    pub custom_metadata: Option<EditableMetadata>,
}

impl PlaylistItemOverride {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.custom_video_format_id.is_some()
            || self.custom_audio_format_id.is_some()
            || self.custom_container.is_some()
            || self.custom_sub_en.is_some()
            || self.custom_sub_id.is_some()
            || self.custom_metadata.is_some()
    }

    #[must_use]
    pub fn badge_token(&self) -> &'static str {
        if !self.is_active() {
            "[GLOBAL]"
        } else if self.custom_metadata.is_some()
            && self.custom_video_format_id.is_none()
            && self.custom_container.is_none()
        {
            "[* META]"
        } else {
            "[* OVER]"
        }
    }
}

/// Represents a single entry inside a Video Playlist or Audio Album/Playlist.
/// Notice that each item can have its own aspect ratio (`1:1` Art Track vs `16:9` VEVO MV),
/// its own available resolutions, and its own `EN`/`ID` subtitle availability!
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistItemEntry {
    pub index_1_based: usize,
    pub media_id: String,
    pub canonical_url: String,
    pub title: String,
    pub channel_or_artist: Option<String>,
    pub duration_secs: Option<u64>,
    pub selected_for_download: bool,
    pub hydrated: bool,
    pub aspect_ratio: AspectRatioKind,
    pub best_thumbnail_url: Option<String>,
    pub available_formats: Vec<StreamCandidate>,
    pub manual_sub_keys: Vec<String>,
    pub auto_sub_keys: Vec<String>,
    pub already_in_history_ledger: bool,
}

/// Complete state of an inspected Playlist or Album with two-tier configuration (`Global` + `ItemOverride`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistWorkspace {
    pub playlist_id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub global_policy: GlobalPlaylistPolicy,
    pub entries: Vec<PlaylistItemEntry>,
    pub item_overrides: BTreeMap<String, PlaylistItemOverride>,
}

impl PlaylistWorkspace {
    /// Resolves the effective container for a specific playlist entry (ItemOverride > GlobalPolicy).
    #[must_use]
    pub fn effective_container(&self, media_id: &str) -> ContainerFormat {
        self.item_overrides
            .get(media_id)
            .and_then(|ov| ov.custom_container)
            .unwrap_or(self.global_policy.container)
    }

    /// Resolves the effective subtitle plan for a specific playlist entry, accounting for
    /// that item's actual available `manual_sub_keys` and `auto_sub_keys`.
    #[must_use]
    pub fn effective_subtitle_plan(&self, entry: &PlaylistItemEntry) -> SubtitleResolutionPlan {
        let ov = self.item_overrides.get(&entry.media_id);
        let want_en = ov
            .and_then(|o| o.custom_sub_en)
            .unwrap_or(self.global_policy.want_sub_en);
        let want_id = ov
            .and_then(|o| o.custom_sub_id)
            .unwrap_or(self.global_policy.want_sub_id);
        let container = self.effective_container(&entry.media_id);

        SubtitleResolutionPlan::resolve(
            &entry.manual_sub_keys,
            &entry.auto_sub_keys,
            want_en,
            want_id,
            self.global_policy.allow_auto_sub_fallback,
            container,
        )
    }
}

fn sanitize_folder_name(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            _ => c,
        })
        .collect();
    if cleaned.is_empty() {
        "mvd_playlist".to_string()
    } else {
        cleaned
    }
}
