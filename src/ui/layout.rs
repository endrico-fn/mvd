use crate::domain::media::MediaArchetype;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Computed conditional regions for `Page [1] EXTRACTOR & PREVIEW`.
/// Enforces the rule: Preview is ALWAYS on the Left, adapting sub-panes to the 6 `MediaArchetype`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionalExtractorLayout {
    pub url_bar_area: Rect,
    pub left_thumbnail_area: Rect,
    /// Present ONLY for `SingleAudio` and `AudioPlaylist` (`LRCLIB` Lyrics below thumbnail)
    pub left_lyrics_area: Option<Rect>,
    /// Present ONLY for `SingleVideo` and `VideoPlaylist` (Chapters & `EN`/`ID` Subtitle source summary)
    pub left_chapters_subs_area: Option<Rect>,
    pub right_top_summary_area: Rect,
    pub right_main_inspector_area: Rect,
}

impl ConditionalExtractorLayout {
    #[must_use]
    pub fn compute(body_area: Rect, archetype: MediaArchetype) -> Self {
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(10)])
            .split(body_area);

        let url_bar_area = vert[0];
        let main_area = vert[1];

        // Image / GIF archetypes get a wider 45% left visual preview column; Audio/Video get 38%
        let (left_pct, right_pct) = match archetype {
            MediaArchetype::StaticImage | MediaArchetype::AnimatedGif => (45, 55),
            _ => (38, 62),
        };

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(left_pct),
                Constraint::Length(1), // 1-cell crisp gap between Left Preview and Right Inspector
                Constraint::Percentage(right_pct),
            ])
            .split(main_area);

        let left_col = cols[0];
        let right_col = cols[2];

        // Split Left Column conditionally based on MediaArchetype
        let (left_thumbnail_area, left_lyrics_area, left_chapters_subs_area) =
            if archetype.shows_lyrics_pane() {
                let split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .split(left_col);
                (split[0], Some(split[1]), None)
            } else if archetype.shows_chapters_and_subs_pane() {
                let split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(left_col);
                (split[0], None, Some(split[1]))
            } else {
                // StaticImage & AnimatedGif use 100% of the Left Column for high-res visual preview
                (left_col, None, None)
            };

        // Split Right Column into Top Summary/Global Policy Bar + Main Stream/Playlist Inspector
        let right_top_len = if archetype.is_collection() { 4 } else { 6 };
        let right_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(right_top_len), Constraint::Min(6)])
            .split(right_col);

        Self {
            url_bar_area,
            left_thumbnail_area,
            left_lyrics_area,
            left_chapters_subs_area,
            right_top_summary_area: right_split[0],
            right_main_inspector_area: right_split[1],
        }
    }
}
