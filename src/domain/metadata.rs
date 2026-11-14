use crate::domain::media::{ContainerFormat, MediaArchetype};
use serde::{Deserialize, Serialize};

/// Editable metadata fields strictly scoped to what the target media archetype & container support.
/// Note: Playlists as a top-level header do NOT edit individual media tags (only output subfolder);
/// individual items inside a playlist can override their own `EditableMetadata`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditableMetadata {
    pub title: String,
    pub artist_or_creator: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub release_year: Option<String>,
    pub genre: Option<String>,
    pub comment_or_description: Option<String>,
    pub custom_output_stem: Option<String>,
    pub embed_cover_art: bool,
    pub embed_synced_lyrics: bool,
    pub embed_chapters: bool,
}

/// Enumerates which metadata fields are legally editable for a given `MediaArchetype` and `ContainerFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataFieldKind {
    Title,
    ArtistOrCreator,
    Album,
    TrackNumber,
    ReleaseYear,
    Genre,
    Comment,
    OutputFilenameStem,
}

impl EditableMetadata {
    #[must_use]
    pub fn default_for_title(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            artist_or_creator: None,
            album: None,
            album_artist: None,
            track_number: None,
            track_total: None,
            release_year: None,
            genre: None,
            comment_or_description: None,
            custom_output_stem: None,
            embed_cover_art: true,
            embed_synced_lyrics: true,
            embed_chapters: true,
        }
    }

    /// Returns the exact list of metadata fields supported by the current archetype & container.
    /// Top-level playlists return an empty slice because playlist-level editing only renames the subfolder;
    /// per-item editing inside a playlist uses `SingleAudio` or `SingleVideo` rules.
    #[must_use]
    pub fn supported_fields_for(
        archetype: MediaArchetype,
        container: ContainerFormat,
    ) -> &'static [MetadataFieldKind] {
        match archetype {
            MediaArchetype::VideoPlaylist | MediaArchetype::AudioPlaylist => &[],
            MediaArchetype::SingleAudio => &[
                MetadataFieldKind::Title,
                MetadataFieldKind::ArtistOrCreator,
                MetadataFieldKind::Album,
                MetadataFieldKind::TrackNumber,
                MetadataFieldKind::ReleaseYear,
                MetadataFieldKind::Genre,
                MetadataFieldKind::OutputFilenameStem,
            ],
            MediaArchetype::SingleVideo => match container {
                ContainerFormat::Mkv | ContainerFormat::Mp4 => &[
                    MetadataFieldKind::Title,
                    MetadataFieldKind::ArtistOrCreator,
                    MetadataFieldKind::ReleaseYear,
                    MetadataFieldKind::Genre,
                    MetadataFieldKind::Comment,
                    MetadataFieldKind::OutputFilenameStem,
                ],
                _ => &[
                    MetadataFieldKind::Title,
                    MetadataFieldKind::ArtistOrCreator,
                    MetadataFieldKind::OutputFilenameStem,
                ],
            },
            MediaArchetype::StaticImage | MediaArchetype::AnimatedGif => &[
                MetadataFieldKind::Title,
                MetadataFieldKind::ArtistOrCreator,
                MetadataFieldKind::OutputFilenameStem,
            ],
        }
    }

    /// Sanitizes the effective title/stem into a filesystem-safe filename (without extension).
    #[must_use]
    pub fn safe_filename_stem(&self, media_id: &str) -> String {
        let base = self
            .custom_output_stem
            .as_deref()
            .unwrap_or(&self.title)
            .trim();
        let cleaned: String = base
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
                _ => c,
            })
            .collect();
        let normalized = if cleaned.is_empty() {
            "mvd_media"
        } else {
            cleaned.as_str()
        };
        format!("{normalized} [{media_id}]")
    }
}
