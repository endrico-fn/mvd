use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpotifyTrackMetadata {
    pub spotify_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: u64,
    pub release_date: Option<String>,
    pub cover_art_640_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YtSearchAudioCandidate {
    pub video_id: String,
    pub title: String,
    pub channel: String,
    pub duration_secs: u64,
}

/// Zero-API-Key Spotify Track Resolver + Duration-Scored YouTube Music Audio Bridge (`T-07`).
pub struct SpotifyBridge;

impl SpotifyBridge {
    /// Scores `ytsearch` candidates by duration proximity (`|yt_dur - sp_dur|`) and `- Topic` channel bonus.
    /// Verified on `T-07` (`7MtVPRGtZl6rPjMfLoI3Lh`, `218.287s`) to select the `219s` audio track (`0.7s` delta)
    /// and reject the `560s` age-gated double music video (`LrSX_OcpeJg`, `341.7s` delta).
    #[must_use]
    pub fn pick_best_audio_match<'a>(
        spotify: &SpotifyTrackMetadata,
        candidates: &'a [YtSearchAudioCandidate],
    ) -> Option<&'a YtSearchAudioCandidate> {
        let target_secs = (spotify.duration_ms as f64) / 1000.0;
        candidates.iter().min_by(|a, b| {
            let score_a = Self::candidate_penalty_score(target_secs, a);
            let score_b = Self::candidate_penalty_score(target_secs, b);
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    #[must_use]
    pub fn candidate_penalty_score(
        target_duration_secs: f64,
        candidate: &YtSearchAudioCandidate,
    ) -> f64 {
        let dur_diff = (candidate.duration_secs as f64 - target_duration_secs).abs();
        let topic_bonus = if candidate.channel.ends_with(" - Topic") {
            -12.0
        } else {
            0.0
        };
        // Heavily penalize candidates whose duration differs by more than 25 seconds (e.g. double MVs or live versions)
        let mismatch_penalty = if dur_diff > 25.0 { dur_diff * 4.0 } else { 0.0 };
        dur_diff + mismatch_penalty + topic_bonus
    }
}
