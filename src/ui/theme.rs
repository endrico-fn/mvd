use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

/// Utilitarian color palette tokens.
pub const COLOR_BORDER_DIM: Color = Color::Rgb(74, 77, 82); // #4A4D52
pub const COLOR_ACCENT_EMERALD: Color = Color::Rgb(40, 184, 119); // #28b877
pub const COLOR_TEXT_PRIMARY: Color = Color::Rgb(236, 239, 244); // #ECEFF4
pub const COLOR_TEXT_MUTED: Color = Color::Rgb(136, 142, 153); // #888E99
pub const COLOR_STATUS_INFO: Color = Color::Rgb(59, 130, 246); // #3B82F6
pub const COLOR_STATUS_WARN: Color = Color::Rgb(245, 158, 11); // #F59E0B
pub const COLOR_STATUS_ERROR: Color = Color::Rgb(239, 68, 68); // #EF4444

/// JetBrains Mono compatible geometric symbols (Strictly ZERO emoji).
pub const SYM_CURSOR: &str = "▸";
pub const SYM_BULLET: &str = "▪";
pub const SYM_CHECKED: &str = "[x]";
pub const SYM_UNCHECKED: &str = "[ ]";
pub const SYM_OVERRIDE: &str = "[*]";

/// Builds a consistent Utilitarian panel with:
/// - Full thin border with sharp 90-degree corners (`BorderType::Plain`: `┌ ─ ┐ │ └ ─ ┘`)
/// - Centered token title (`─[ TITLE ]─`)
/// - Conditional border color (`#28b877` when focused, `#4A4D52` when unfocused)
#[must_use]
pub fn sharp_centered_panel(token_title: &str, is_focused: bool) -> Block<'_> {
    let border_color = if is_focused {
        COLOR_ACCENT_EMERALD
    } else {
        COLOR_BORDER_DIM
    };
    let title_style = if is_focused {
        Style::default()
            .fg(COLOR_ACCENT_EMERALD)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT_PRIMARY)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .title(format!("─[ {token_title} ]─"))
        .title_alignment(Alignment::Center)
        .title_style(title_style)
}
