use crate::domain::media::AspectRatioKind;
use ratatui::layout::Rect;

/// Terminal cell pixel geometry queried via Kitty `TIOCGWINSZ` (defaults to 10x20px if unavailable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPixelGeometry {
    pub cell_width_px: u16,
    pub cell_height_px: u16,
}

impl Default for CellPixelGeometry {
    fn default() -> Self {
        Self {
            cell_width_px: 10,
            cell_height_px: 20,
        }
    }
}

impl CellPixelGeometry {
    /// Queries stdout `TIOCGWINSZ` via `rustix` to get exact pixel dimensions per character cell in Kitty.
    #[must_use]
    pub fn probe_kitty_stdout() -> Self {
        if let Ok(ws) = rustix::termios::tcgetwinsize(std::io::stdout()) {
            if ws.ws_col > 0 && ws.ws_row > 0 && ws.ws_xpixel > 0 && ws.ws_ypixel > 0 {
                let cw = (ws.ws_xpixel / ws.ws_col).max(4);
                let ch = (ws.ws_ypixel / ws.ws_row).max(8);
                return Self {
                    cell_width_px: cw,
                    cell_height_px: ch,
                };
            }
        }
        Self::default()
    }

    /// Computes a centered sub-`Rect` inside `available_inner` that preserves `aspect`
    /// (`16:9` VEVO/YouTube, `1:1` Art Track/Spotify, `9:16` Shorts/Reels, `4:5` IG, `2:3` Pinterest)
    /// without stretching or cropping.
    #[must_use]
    pub fn fit_centered_rect(self, available_inner: Rect, aspect: AspectRatioKind) -> Rect {
        if available_inner.width <= 2 || available_inner.height <= 1 {
            return available_inner;
        }

        let target_ratio = aspect.numeric_ratio();
        let avail_px_w = f64::from(available_inner.width) * f64::from(self.cell_width_px);
        let avail_px_h = f64::from(available_inner.height) * f64::from(self.cell_height_px);

        let (fit_px_w, fit_px_h) = if avail_px_w / avail_px_h > target_ratio {
            let h = avail_px_h;
            let w = h * target_ratio;
            (w, h)
        } else {
            let w = avail_px_w;
            let h = w / target_ratio;
            (w, h)
        };

        let target_cols = ((fit_px_w / f64::from(self.cell_width_px)).round() as u16)
            .clamp(1, available_inner.width);
        let target_rows = ((fit_px_h / f64::from(self.cell_height_px)).round() as u16)
            .clamp(1, available_inner.height);

        let offset_x = (available_inner.width.saturating_sub(target_cols)) / 2;
        let offset_y = (available_inner.height.saturating_sub(target_rows)) / 2;

        Rect {
            x: available_inner.x + offset_x,
            y: available_inner.y + offset_y,
            width: target_cols,
            height: target_rows,
        }
    }
}
