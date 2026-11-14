use crate::domain::error::MvdError;
use crate::domain::media::{MediaArchetype, PlatformKind};
use url::Url;

const TRACKING_QUERY_KEYS: &[&str] = &[
    "si",
    "feature",
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "igshid",
    "s",
    "t",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMediaUrl {
    pub raw_input: String,
    pub normalized_url: String,
    pub host_domain: String,
    pub platform: PlatformKind,
    pub initial_archetype: MediaArchetype,
    pub extracted_id_hint: Option<String>,
}

pub struct UrlParser;

impl UrlParser {
    /// Parses, sanitizes tracking parameters (`?si=...`, `utm_*`), classifies platform,
    /// and determines the initial `MediaArchetype` for the 6-condition TUI layout.
    pub fn parse_and_normalize(raw: &str) -> Result<ParsedMediaUrl, MvdError> {
        let trimmed = raw.trim();
        let parsed = Url::parse(trimmed).map_err(|e| MvdError::InvalidUrl {
            input: trimmed.to_string(),
            reason: e.to_string(),
        })?;

        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(MvdError::InvalidUrl {
                input: trimmed.to_string(),
                reason: format!("Unsupported URL scheme '{scheme}'"),
            });
        }

        let host = parsed
            .host_str()
            .ok_or_else(|| MvdError::InvalidUrl {
                input: trimmed.to_string(),
                reason: "Missing host".to_string(),
            })?
            .to_lowercase();

        let platform = Self::classify_platform(&host, parsed.path());
        let mut clean_url = parsed.clone();

        // Preserve functional query parameters (v, list, drli, id) while stripping tracking tokens
        let kept_pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .filter(|(k, _)| !TRACKING_QUERY_KEYS.contains(&k.as_ref()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        if kept_pairs.is_empty() {
            clean_url.set_query(None);
        } else {
            clean_url.query_pairs_mut().clear().extend_pairs(kept_pairs);
        }

        let initial_archetype = Self::infer_archetype(platform, &clean_url);
        let extracted_id_hint = Self::extract_id_hint(platform, &clean_url);
        let host_domain = Self::registrable_cookie_domain(&host);

        Ok(ParsedMediaUrl {
            raw_input: trimmed.to_string(),
            normalized_url: clean_url.to_string(),
            host_domain,
            platform,
            initial_archetype,
            extracted_id_hint,
        })
    }

    #[must_use]
    pub fn classify_platform(host: &str, path: &str) -> PlatformKind {
        if host == "music.youtube.com" {
            PlatformKind::YouTubeMusic
        } else if host.ends_with("youtube.com") || host == "youtu.be" {
            if path.starts_with("/shorts/") {
                PlatformKind::YouTubeShorts
            } else {
                PlatformKind::YouTube
            }
        } else if host.ends_with("spotify.com") {
            PlatformKind::Spotify
        } else if host == "x.com" || host.ends_with("twitter.com") || host == "t.co" {
            PlatformKind::XTwitter
        } else if host.ends_with("reddit.com") || host == "v.redd.it" || host == "i.redd.it" {
            PlatformKind::Reddit
        } else if host.ends_with("instagram.com") {
            PlatformKind::Instagram
        } else if host.ends_with("facebook.com") || host == "fb.watch" {
            PlatformKind::Facebook
        } else if host.ends_with("pinterest.com")
            || host == "pin.it"
            || host.ends_with("pinimg.com")
        {
            PlatformKind::Pinterest
        } else if host.ends_with("soundcloud.com") {
            PlatformKind::SoundCloud
        } else if path.ends_with(".m3u8") || path.ends_with(".mpd") {
            PlatformKind::GenericHttpOrHls
        } else {
            PlatformKind::AdultOrSensitiveSandbox
        }
    }

    #[must_use]
    pub fn infer_archetype(platform: PlatformKind, url: &Url) -> MediaArchetype {
        let path = url.path().to_lowercase();
        let has_list = url.query_pairs().any(|(k, _)| k == "list");
        let has_video_v = url.query_pairs().any(|(k, _)| k == "v");

        if path.ends_with(".gif") {
            return MediaArchetype::AnimatedGif;
        }
        if path.ends_with(".png")
            || path.ends_with(".jpg")
            || path.ends_with(".jpeg")
            || path.ends_with(".webp")
        {
            return MediaArchetype::StaticImage;
        }

        match platform {
            PlatformKind::YouTubeMusic => {
                if path.contains("/playlist") || (has_list && !has_video_v) {
                    MediaArchetype::AudioPlaylist
                } else {
                    MediaArchetype::SingleAudio
                }
            }
            PlatformKind::Spotify => {
                if path.contains("/album/") || path.contains("/playlist/") {
                    MediaArchetype::AudioPlaylist
                } else {
                    MediaArchetype::SingleAudio
                }
            }
            PlatformKind::SoundCloud => {
                if path.contains("/sets/") {
                    MediaArchetype::AudioPlaylist
                } else {
                    MediaArchetype::SingleAudio
                }
            }
            PlatformKind::YouTube => {
                if path.contains("/playlist") || (has_list && !has_video_v) {
                    MediaArchetype::VideoPlaylist
                } else {
                    MediaArchetype::SingleVideo
                }
            }
            PlatformKind::Pinterest => MediaArchetype::StaticImage,
            PlatformKind::Instagram => {
                if path.contains("/reel/") || path.contains("/reels/") {
                    MediaArchetype::SingleVideo
                } else {
                    MediaArchetype::StaticImage
                }
            }
            _ => MediaArchetype::SingleVideo,
        }
    }

    #[must_use]
    pub fn extract_id_hint(platform: PlatformKind, url: &Url) -> Option<String> {
        if let Some((_, v)) = url.query_pairs().find(|(k, _)| k == "v") {
            return Some(v.into_owned());
        }
        if let Some((_, list)) = url.query_pairs().find(|(k, _)| k == "list") {
            return Some(list.into_owned());
        }
        if platform == PlatformKind::YouTube && url.host_str() == Some("youtu.be") {
            return url
                .path_segments()
                .and_then(|mut s| s.next())
                .filter(|s| !s.is_empty())
                .map(ToString::to_string);
        }
        url.path_segments()
            .and_then(Iterator::last)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
    }

    #[must_use]
    pub fn registrable_cookie_domain(host: &str) -> String {
        if host.ends_with("youtube.com") || host == "youtu.be" {
            "youtube.com".to_string()
        } else if host.ends_with("twitter.com") || host == "x.com" {
            "x.com".to_string()
        } else if host.ends_with("reddit.com") || host.ends_with("redd.it") {
            "reddit.com".to_string()
        } else if host.ends_with("instagram.com") {
            "instagram.com".to_string()
        } else if host.ends_with("facebook.com") || host == "fb.watch" {
            "facebook.com".to_string()
        } else if host.ends_with("pinterest.com") || host == "pin.it" {
            "pinterest.com".to_string()
        } else {
            host.trim_start_matches("www.").to_string()
        }
    }
}
