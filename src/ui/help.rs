use crate::ui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, theme: &Theme) {
    let area = centered_rect(64, 28, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled(" DTop — Keybindings (? or F1 to close) ", theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into two columns
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left = vec![
        key_line(theme, "Global", ""),
        key_line(theme, "  q / Ctrl-C", "Quit"),
        key_line(theme, "  Esc / h",    "Back / Dashboard"),
        key_line(theme, "  Tab",        "Focus next panel"),
        key_line(theme, "  Shift-Tab",  "Focus prev panel"),
        key_line(theme, "  ↑↓ / j k",  "Select / scroll"),
        key_line(theme, "  Enter / l",  "Drill-down / confirm"),
        key_line(theme, "  PageUp/Dn",  "Scroll list"),
        Line::from(""),
        key_line(theme, "View switching", ""),
        key_line(theme, "  F2",  "Process I/O view"),
        key_line(theme, "  F3",  "Filesystem overview"),
        key_line(theme, "  F4",  "RAID / LVM / ZFS view"),
        key_line(theme, "  F5",  "NFS mount latency view"),
        key_line(theme, "  Esc", "Return to Dashboard"),
        Line::from(""),
        key_line(theme, "Dashboard", ""),
        key_line(theme, "  Enter", "Open device detail"),
        key_line(theme, "  s",     "Force SMART refresh"),
        key_line(theme, "  p",     "Cycle layout preset"),
        key_line(theme, "  w",     "Cycle history window (60s/5m/1h)"),
    ];

    let right = vec![
        key_line(theme, "Appearance", ""),
        key_line(theme, "  t", "Cycle theme (Default → Dracula → Gruvbox → Nord)"),
        Line::from(""),
        key_line(theme, "Process I/O (F2)", ""),
        key_line(theme, "  s",    "Cycle sort column"),
        key_line(theme, "  ↑↓",  "Select process"),
        Line::from(""),
        key_line(theme, "Filesystem (F3)", ""),
        key_line(theme, "  ↑↓",  "Scroll table"),
        Line::from(""),
        key_line(theme, "Volume Manager (F4)", ""),
        key_line(theme, "  ↑↓",  "Scroll list"),
        Line::from(""),
        key_line(theme, "Mouse", ""),
        key_line(theme, "  Click",      "Select device"),
        key_line(theme, "  Scroll",     "Scroll active panel"),
        Line::from(""),
        key_line(theme, "Sort modes (F2 s)", ""),
        key_line(theme, "  Write/s → Read/s → Total → PID → Name", ""),
    ];

    f.render_widget(Paragraph::new(left), cols[0]);
    f.render_widget(Paragraph::new(right), cols[1]);
}

fn key_line<'a>(theme: &Theme, key: &'a str, desc: &'a str) -> Line<'a> {
    if desc.is_empty() {
        // Section header
        Line::from(vec![
            Span::styled(key, theme.title),
        ])
    } else {
        Line::from(vec![
            Span::styled(format!("{:<22}", key), theme.footer_key),
            Span::styled(desc, theme.text_dim),
        ])
    }
}

/// Returns a centered Rect of `pct_w`% width and `pct_h`% height,
/// but capped at the available area.
fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Helper that splits and discards to achieve centering by percentage (unused but kept for reference).
#[allow(dead_code)]
fn centered_pct(pct_w: u16, pct_h: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_h) / 2),
            Constraint::Percentage(pct_h),
            Constraint::Percentage((100 - pct_h) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_w) / 2),
            Constraint::Percentage(pct_w),
            Constraint::Percentage((100 - pct_w) / 2),
        ])
        .split(popup_layout[1])[1]
}
