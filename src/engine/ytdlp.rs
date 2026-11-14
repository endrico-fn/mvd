use crate::domain::media::AspectRatioKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct YtDlpThumbnailJson {
    pub id: Option<String>,
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub preference: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedThumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub aspect_kind: AspectRatioKind,
}

/// Smart Thumbnail Selector built from empirical testing on `T-02`, `T-03`, `T-04`, `T-05`, `T-06`.
pub struct SmartThumbnailSelector;

impl SmartThumbnailSelector {
    /// Selects the most accurate thumbnail while respecting the "Apa Adanya / Native As-Is" rule:
    /// 1. If the media is a pure `1:1` Art Track (`is_art_track == true`, e.g., `5xx9gTEQaiQ` where video stream
    ///    is `1080x1080` or uploader ends with ` - Topic`), it prioritizes true square `w == h` thumbnails
    ///    (`yt3.googleusercontent.com` `544x544`) and ignores `hq720.jpg` (`1280x720` with black sidebars!).
    /// 2. If the media is a VEVO / Artist Music Video (`sgm3F5D1yz8`, `LrSX_OcpeJg`) or community video (`WHVupRkTdqo`),
    ///    it preserves the native `16:9` (`1920x1080` / `1280x720`) thumbnail as-is without cropping to `1:1`.
    #[must_use]
    pub fn select_best(
        thumbnails: &[YtDlpThumbnailJson],
        is_art_track_1x1: bool,
        is_vevo_or_landscape_mv: bool,
    ) -> Option<SelectedThumbnail> {
        if thumbnails.is_empty() {
            return None;
        }

        if is_art_track_1x1 && !is_vevo_or_landscape_mv {
            // Find highest-resolution true square (w == h) cover art
            if let Some(sq) = thumbnails
                .iter()
                .filter(|t| matches!((t.width, t.height), (Some(w), Some(h)) if w == h && w >= 180))
                .max_by_key(|t| t.width.unwrap_or(0))
            {
                let w = sq.width.unwrap_or(544);
                let h = sq.height.unwrap_or(544);
                return Some(SelectedThumbnail {
                    url: sq.url.clone(),
                    width: w,
                    height: h,
                    aspect_kind: AspectRatioKind::Square1x1,
                });
            }
        }

        // Otherwise pick the highest-area thumbnail with known dimensions (preserving VEVO 16:9, Shorts 9:16, IG 4:5)
        let best = thumbnails
            .iter()
            .filter(|t| t.width.is_some() && t.height.is_some())
            .max_by_key(|t| {
                let area = u64::from(t.width.unwrap_or(0)) * u64::from(t.height.unwrap_or(0));
                (area, t.preference.unwrap_or(-100))
            })
            .or_else(|| thumbnails.last())?;

        let w = best.width.unwrap_or(1280);
        let h = best.height.unwrap_or(720);
        let aspect_kind =
            AspectRatioKind::from_dimensions_and_context(w, h, is_vevo_or_landscape_mv);

        Some(SelectedThumbnail {
            url: best.url.clone(),
            width: w,
            height: h,
            aspect_kind,
        })
    }

    /// Determines whether a YouTube Music item is a pure `1:1` Art Track vs a `16:9` VEVO/Artist Music Video.
    #[must_use]
    pub fn classify_ytmusic_visual_kind(
        channel: Option<&str>,
        album: Option<&str>,
        stream_resolutions: &[String],
        thumbnails: &[YtDlpThumbnailJson],
    ) -> (bool, bool) {
        let has_square_video_stream = stream_resolutions
            .iter()
            .any(|r| r == "1080x1080" || r == "720x720");
        let has_16x9_hd_stream = stream_resolutions
            .iter()
            .any(|r| r == "1920x1080" || r == "3840x2160" || r == "2560x1440");
        let is_topic_channel = channel.is_some_and(|c| c.ends_with(" - Topic"));
        let has_google_square_thumb = thumbnails.iter().any(|t| {
            matches!((t.width, t.height), (Some(w), Some(h)) if w == h)
                && (t.url.contains("googleusercontent.com") || t.url.contains("/s_p/"))
        });

        // If the stream itself is 1080x1080 (like T-05 5xx9gTEQaiQ) or it's a '- Topic' Art Track:
        if has_square_video_stream || is_topic_channel {
            return (true, false);
        }
        // If the stream is 1920x1080 / 4K and NOT a square stream (like VEVO T-04 LrSX_OcpeJg or T-03 sgm3F5D1yz8):
        if has_16x9_hd_stream && album.is_none() {
            return (false, true);
        }
        if has_16x9_hd_stream && !has_square_video_stream {
            return (false, true);
        }

        (has_google_square_thumb, !has_google_square_thumb)
    }
}
