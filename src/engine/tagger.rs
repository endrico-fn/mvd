use crate::domain::metadata::EditableMetadata;
use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::{ItemKey, Tag, TagItem, TagType};
use std::path::Path;

/// Pure-Rust audio metadata, 1:1 cover art, and synced LRCLIB lyrics tagger using `lofty`.
/// Eliminates the need for any external tagging binaries on the host system.
pub struct AudioMetadataTagger;

impl AudioMetadataTagger {
    pub fn write_tags_and_lyrics(
        audio_path: &Path,
        metadata: &EditableMetadata,
        synced_or_plain_lyrics: Option<&str>,
        cover_jpeg_bytes: Option<&[u8]>,
    ) -> Result<(), String> {
        let mut tagged_file = lofty::read_from_path(audio_path).map_err(|e| e.to_string())?;

        let tag_type = tagged_file.primary_tag_type();
        let tag = match tagged_file.primary_tag_mut() {
            Some(existing) => existing,
            None => {
                tagged_file.insert_tag(Tag::new(tag_type));
                tagged_file
                    .primary_tag_mut()
                    .ok_or_else(|| "Failed to initialize primary audio tag".to_string())?
            }
        };

        tag.set_title(metadata.title.clone());
        if let Some(artist) = &metadata.artist_or_creator {
            tag.set_artist(artist.clone());
        }
        if let Some(album) = &metadata.album {
            tag.set_album(album.clone());
        }
        if let Some(track_no) = metadata.track_number {
            tag.set_track(track_no);
        }
        if let Some(genre) = &metadata.genre {
            tag.set_genre(genre.clone());
        }

        if metadata.embed_synced_lyrics {
            if let Some(lrc_text) = synced_or_plain_lyrics {
                if !lrc_text.trim().is_empty() {
                    tag.insert(TagItem::new(
                        ItemKey::Lyrics,
                        lofty::tag::ItemValue::Text(lrc_text.to_string()),
                    ));
                }
            }
        }

        if metadata.embed_cover_art {
            if let Some(bytes) = cover_jpeg_bytes {
                if tag_type != TagType::Id3v1 {
                    let picture = Picture::new_unchecked(
                        PictureType::CoverFront,
                        Some(MimeType::Jpeg),
                        None,
                        bytes.to_vec(),
                    );
                    tag.push_picture(picture);
                }
            }
        }

        tagged_file
            .save_to_path(audio_path, WriteOptions::default())
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
