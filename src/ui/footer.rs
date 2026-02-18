use crate::app::{ActivePanel, ActiveView};
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

const PRESET_NAMES: [&str; 3] = ["Full", "IO-Focus", "Storage"];

pub fn render_footer(
    f: &mut Frame,
    area: Rect,
    panel: &ActivePanel,
    layout_preset: usize,
    theme: &Theme,
    active_view: &ActiveView,
    detail_open: bool,
) {
    let preset_label = PRESET_NAMES[layout_preset.min(2)];
    let base: &[(&str, &str)] = match panel {
        ActivePanel::Devices => &[
            ("q", "Quit"), ("Tab", "Focus"), ("↑↓/jk", "Select"), ("g/G", "Top/Bot"),
            ("Enter/l", "Detail"), ("s", "Sort"), ("f", "Filter"), ("t", "Theme"),
        ],
        ActivePanel::Throughput => &[
            ("q", "Quit"), ("Tab", "Focus"), ("t", "Theme"),
        ],
        ActivePanel::Filesystem => &[
            ("q", "Quit"), ("Tab", "Focus"), ("↑↓", "Scroll"), ("t", "Theme"),
        ],
        ActivePanel::SmartTemp => &[
            ("q", "Quit"), ("Tab", "Focus"), ("t", "Theme"),
        ],
        ActivePanel::Alerts => &[
            ("q", "Quit"), ("Tab", "Focus"), ("a", "Ack all"), ("t", "Theme"),
        ],
        ActivePanel::Detail => &[
            ("Esc/h", "Back"), ("↑↓/jk", "Scroll"), ("w", "Window"),
            ("r", "SMART refresh"), ("b", "Benchmark"), ("x", "SMART test"),
            ("B", "Baseline"), ("D", "Descriptions"), ("t", "Theme"), ("q", "Quit"),
        ],
    };

    let mut spans: Vec<Span> = vec![Span::styled(" ", theme.footer_bg)];

    for (key, desc) in base {
        spans.push(Span::styled(format!(" {} ", key), theme.footer_key));
        spans.push(Span::styled(format!("{}  ", desc), theme.footer_text));
    }

    // Append layout preset indicator (only on dashboard non-detail panels)
    match panel {
        ActivePanel::Detail => {}
        _ => {
            spans.push(Span::styled(format!(" p ", ), theme.footer_key));
            spans.push(Span::styled(format!("{}  ", preset_label), theme.footer_text));
            spans.push(Span::styled(" F5 ", theme.footer_key));
            spans.push(Span::styled("NFS  ", theme.footer_text));
            spans.push(Span::styled(" F6 ", theme.footer_key));
            spans.push(Span::styled("Alerts  ", theme.footer_text));
            spans.push(Span::styled(" ? ", theme.footer_key));
            spans.push(Span::styled("Help  ", theme.footer_text));
        }
    }

    // Context-sensitive hint line
    let hint = match (active_view, detail_open) {
        (ActiveView::AlertLog, _)           => "/ search  s filter  \u{2191}\u{2193} scroll  Esc back",
        (ActiveView::ProcessIO, _)          => "s sort  \u{2191}\u{2193} navigate  Esc back",
        (ActiveView::FilesystemOverview, _) => "\u{2191}\u{2193} scroll  g/G first/last  Esc back",
        (ActiveView::VolumeManager, _)      => "\u{2191}\u{2193} scroll  Esc back",
        (ActiveView::NfsView, _)            => "\u{2191}\u{2193} scroll  g/G first/last  Esc back",
        (ActiveView::Dashboard, true)       => "w window  r SMART  B baseline  b bench  x test  D desc  Esc back",
        (ActiveView::Dashboard, false)      => "f filter  s sort  p layout  a ack  Enter open  t theme  ? help",
    };

    spans.push(Span::styled("  \u{2502}  ", theme.footer_text));
    spans.push(Span::styled(hint, theme.footer_text));

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(theme.footer_bg);
    f.render_widget(para, area);
}
