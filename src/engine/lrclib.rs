use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LrcLine {
    pub timestamp_tag: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsDocument {
    pub lrclib_id: Option<u64>,
    pub track_name: String,
    pub artist_name: String,
    pub is_synced: bool,
    pub is_instrumental: bool,
    pub lines: Vec<LrcLine>,
    pub raw_lrc: String,
}

pub struct LrclibCleaner;

impl LrclibCleaner {
    /// Cleans raw YouTube / YouTube Music uploader & video titles for maximum `lrclib.net` hit rate.
    #[must_use]
    pub fn normalize_artist_and_track(raw_artist: &str, raw_title: &str) -> (String, String) {
        let artist = raw_artist
            .trim()
            .trim_end_matches(" - Topic")
            .trim_end_matches("VEVO")
            .trim()
            .to_string();

        let mut track = raw_title.trim().to_string();
        for noise in [
            "(Official Music Video)",
            "[Official Music Video]",
            "(Official Video)",
            "[Official Video]",
            "(Official Audio)",
            "[Official Audio]",
            "(Official Lyric Video)",
            "(Lyrics)",
            "[HD]",
            "(HD)",
        ] {
            track = track.replace(noise, "");
        }

        // If title has "Artist - Track", split off the redundant artist prefix
        let prefix = format!("{artist} - ");
        if track.to_lowercase().starts_with(&prefix.to_lowercase()) {
            track = track[prefix.len()..].to_string();
        }

        (artist, track.trim().to_string())
    }

    #[must_use]
    pub fn parse_lrc_lines(raw_lrc: &str, is_synced: bool) -> Vec<LrcLine> {
        raw_lrc
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if is_synced && trimmed.starts_with('[') {
                    if let Some(close_idx) = trimmed.find(']') {
                        let tag = trimmed[..=close_idx].to_string();
                        let rest = trimmed[close_idx + 1..].trim().to_string();
                        return LrcLine {
                            timestamp_tag: Some(tag),
                            text: rest,
                        };
                    }
                }
                LrcLine {
                    timestamp_tag: None,
                    text: trimmed.to_string(),
                }
            })
            .collect()
    }
}
