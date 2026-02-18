use crate::app::ActivePanel;
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
            ("q", "Quit"), ("Tab", "Focus"), ("t", "Theme"),
        ],
        ActivePanel::Detail => &[
            ("Esc/h", "Back"), ("↑↓/jk", "Scroll"), ("w", "Window"),
            ("b", "Benchmark"), ("x", "SMART test"), ("t", "Theme"), ("q", "Quit"),
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
            spans.push(Span::styled(" ? ", theme.footer_key));
            spans.push(Span::styled("Help  ", theme.footer_text));
        }
    }

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(theme.footer_bg);
    f.render_widget(para, area);
}
