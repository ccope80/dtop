use crate::alerts::{Alert, Severity};
use crate::app::AlertLogFilter;
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_alert_log_view(
    f: &mut Frame,
    area: Rect,
    entries: &[(String, Alert)],  // newest first
    scroll: usize,
    filter: AlertLogFilter,
    theme: &Theme,
) {
    let filtered: Vec<&(String, Alert)> = entries.iter().filter(|(_, a)| match filter {
        AlertLogFilter::All  => true,
        AlertLogFilter::Crit => a.severity == Severity::Critical,
        AlertLogFilter::Warn => a.severity == Severity::Warning,
    }).collect();

    let filter_label = match filter {
        AlertLogFilter::All  => "All",
        AlertLogFilter::Crit => "Crit",
        AlertLogFilter::Warn => "Warn",
    };

    let total = filtered.len();
    let title = format!(
        " F6 Alert Log — {} entries  [s: filter={}]  [Esc/F6 back] ",
        total, filter_label
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled(title, theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 { return; }

    let mut lines: Vec<Line> = Vec::new();
    for (ts, alert) in &filtered {
        let (sev_str, sev_style) = match alert.severity {
            Severity::Critical => ("[CRIT]", theme.crit),
            Severity::Warning  => ("[WARN]", theme.warn),
            Severity::Info     => ("[INFO]", theme.text_dim),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{}  ", ts), theme.text_dim),
            Span::styled(format!("{:<6}  ", sev_str), sev_style),
            Span::styled(alert.message.clone(), theme.text),
        ]));
    }

    let max_scroll = total.saturating_sub(inner.height as usize);
    let actual_scroll = scroll.min(max_scroll) as u16;

    let para = Paragraph::new(lines).scroll((actual_scroll, 0));
    f.render_widget(para, inner);
}
