use serde::{Deserialize, Serialize};

/// The 6 conditional media layout archetypes that drive the Left-Preview + Right-Inspector TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaArchetype {
    /// Condition 1: Single Audio Track (YT Music, Spotify Track, SoundCloud)
    /// Left: Top Thumbnail (`1:1` Cover or `16:9` VEVO as-is) + Bottom `LRCLIB` Lyrics
    /// Right: Editable Audio Metadata + Audio Codec/Bitrate Selector
    SingleAudio,

    /// Condition 2: Audio Playlist / Music Album (`OLAK5uy_...`, YT Music Playlist, Spotify Album)
    /// Left: Highlighted Track Thumbnail (`1:1` or `16:9` as-is) + Highlighted Track `LRCLIB` Lyrics
    /// Right: Global Album Rule Bar + Selectable Tracklist Table with per-track `[*]` override
    AudioPlaylist,

    /// Condition 3: Single Video (YouTube, X/Twitter Video, Reddit Video, Facebook, Adult/HLS)
    /// Left: Native Aspect-Ratio Kitty Thumbnail (`16:9`, `9:16`, `4:5`) + Chapters & `EN`/`ID` Sub Summary
    /// Right: Editable Video Metadata + Video/Audio Stream Matrix + `MKV`/`MP4` & Sub Selector
    SingleVideo,

    /// Condition 4: Video Playlist / Series (YouTube Course/Series, Multi-Video Post)
    /// Left: Highlighted Episode Thumbnail + Episode Chapters & `EN`/`ID` Subtitle Status
    /// Right: Global Ceiling Policy Bar + Episode Checklist Table with per-episode `[*]` override
    VideoPlaylist,

    /// Condition 5: Static Image or Multi-Image Carousel (Instagram Post/Carousel, Pinterest Pin, X Image, Reddit Gallery)
    /// Left: Full-height Native Aspect-Ratio Kitty Preview + Slide Counter `[1/N]`
    /// Right: Image Resolution/Format Inspector + EXIF/Filename Editor
    StaticImage,

    /// Condition 6: Animated GIF or Short Silent Loop (Reddit GIF, X Loop, Pinterest GIF)
    /// Left: Full-height Native Ratio Preview + FPS/Loop Telemetry
    /// Right: Output Container Selector (`.gif`, lossless `.mp4`, `.webm`) + Filename Editor
    AnimatedGif,
}

impl MediaArchetype {
    #[must_use]
    pub const fn shows_lyrics_pane(self) -> bool {
        matches!(self, Self::SingleAudio | Self::AudioPlaylist)
    }

    #[must_use]
    pub const fn shows_chapters_and_subs_pane(self) -> bool {
        matches!(self, Self::SingleVideo | Self::VideoPlaylist)
    }

    #[must_use]
    pub const fn is_collection(self) -> bool {
        matches!(self, Self::AudioPlaylist | Self::VideoPlaylist)
    }

    #[must_use]
    pub const fn badge_token(self) -> &'static str {
        match self {
            Self::SingleAudio => "AUDIO TRACK",
            Self::AudioPlaylist => "AUDIO ALBUM / PLAYLIST",
            Self::SingleVideo => "SINGLE VIDEO",
            Self::VideoPlaylist => "VIDEO PLAYLIST",
            Self::StaticImage => "IMAGE / GALLERY",
            Self::AnimatedGif => "ANIMATED GIF / LOOP",
        }
    }
}

/// Detected source platform for smart aspect ratio, cookie scoping, and extractor selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformKind {
    YouTube,
    YouTubeMusic,
    YouTubeShorts,
    Spotify,
    XTwitter,
    Reddit,
    Instagram,
    Facebook,
    Pinterest,
    SoundCloud,
    AdultOrSensitiveSandbox,
    GenericHttpOrHls,
}

impl PlatformKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::YouTube => "youtube",
            Self::YouTubeMusic => "youtube:music",
            Self::YouTubeShorts => "youtube:shorts",
            Self::Spotify => "spotify:bridge",
            Self::XTwitter => "x:twitter",
            Self::Reddit => "reddit",
            Self::Instagram => "instagram",
            Self::Facebook => "facebook",
            Self::Pinterest => "pinterest",
            Self::SoundCloud => "soundcloud",
            Self::AdultOrSensitiveSandbox => "sandbox:hls",
            Self::GenericHttpOrHls => "direct:stream",
        }
    }

    /// Suggests whether Ephemeral Incognito Mode (`[I]`) should be auto-recommended.
    #[must_use]
    pub const fn recommends_ephemeral_privacy(self) -> bool {
        matches!(self, Self::AdultOrSensitiveSandbox)
    }
}

/// Smart thumbnail aspect ratio classification ("Apa Adanya / Native As-Is" compliant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AspectRatioKind {
    /// `16:9` Standard Landscape (YouTube, VEVO Music Videos on YT Music, X Landscape)
    Landscape16x9,
    /// `1:1` Pure Square (YT Music Art Tracks `w==h`, Spotify Covers, Instagram Square)
    Square1x1,
    /// `9:16` Vertical (Instagram Reels, YouTube Shorts, TikTok, Stories)
    Vertical9x16,
    /// `4:5` or `4:3` Portrait (Instagram Portrait, Reddit/Facebook Feed)
    Portrait4x5,
    /// `2:3` Vertical Pin (Pinterest High-Res Pins)
    VerticalPin2x3,
    /// Arbitrary native dimensions preserved without cropping
    NativeExact { width: u32, height: u32 },
}

impl AspectRatioKind {
    #[must_use]
    pub fn from_dimensions_and_context(
        width: u32,
        height: u32,
        is_vevo_or_music_video: bool,
    ) -> Self {
        if width == 0 || height == 0 {
            return Self::Landscape16x9;
        }
        // Never force-crop a VEVO/Artist music video into 1:1
        if is_vevo_or_music_video && width > height {
            return Self::Landscape16x9;
        }
        if width == height {
            return Self::Square1x1;
        }
        let ratio = f64::from(width) / f64::from(height);
        if (ratio - 1.0).abs() <= 0.08 {
            Self::Square1x1
        } else if ratio >= 1.55 {
            Self::Landscape16x9
        } else if ratio <= 0.60 {
            Self::Vertical9x16
        } else if (0.61..=0.72).contains(&ratio) {
            Self::VerticalPin2x3
        } else if (0.73..=0.88).contains(&ratio) {
            Self::Portrait4x5
        } else {
            Self::NativeExact { width, height }
        }
    }

    #[must_use]
    pub const fn token_label(self) -> &'static str {
        match self {
            Self::Landscape16x9 => "16:9 LANDSCAPE",
            Self::Square1x1 => "1:1 SQUARE ART",
            Self::Vertical9x16 => "9:16 VERTICAL",
            Self::Portrait4x5 => "4:5 PORTRAIT",
            Self::VerticalPin2x3 => "2:3 PIN VERTICAL",
            Self::NativeExact { .. } => "NATIVE AS-IS",
        }
    }

    #[must_use]
    pub fn numeric_ratio(self) -> f64 {
        match self {
            Self::Landscape16x9 => 16.0 / 9.0,
            Self::Square1x1 => 1.0,
            Self::Vertical9x16 => 9.0 / 16.0,
            Self::Portrait4x5 => 4.0 / 5.0,
            Self::VerticalPin2x3 => 2.0 / 3.0,
            Self::NativeExact { width, height } => {
                if height == 0 {
                    16.0 / 9.0
                } else {
                    f64::from(width) / f64::from(height)
                }
            }
        }
    }
}

/// Target output container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerFormat {
    /// Default expert video container (supports AV1/VP9/H264 + Opus/FLAC + SRT/ASS + Chapters + Cover Attachment)
    Mkv,
    /// Compatibility video container (triggers External Sidecar `.srt` + optional `mov_text`)
    Mp4,
    Webm,
    Opus,
    Flac,
    M4a,
    Mp3,
    Gif,
    Png,
    Jpg,
}

impl ContainerFormat {
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Mkv => "mkv",
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
            Self::Opus => "opus",
            Self::Flac => "flac",
            Self::M4a => "m4a",
            Self::Mp3 => "mp3",
            Self::Gif => "gif",
            Self::Png => "png",
            Self::Jpg => "jpg",
        }
    }
}

/// Individual video, audio, or image stream entry extracted from `yt-dlp` JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamCandidate {
    pub format_id: String,
    pub ext: String,
    pub resolution_label: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub abr_kbps: Option<u32>,
    pub tbr_kbps: Option<u32>,
    pub filesize_bytes: Option<u64>,
}
