use crate::alerts::{Alert, Severity};
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::collections::VecDeque;

pub fn render_alerts_panel(
    f: &mut Frame,
    area: Rect,
    alerts: &[Alert],
    history: &VecDeque<(String, Alert)>,
    focused: bool,
    theme: &Theme,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let alert_count = alerts.len();
    let title = if alert_count > 0 {
        format!("5 Alerts  ({} active)", alert_count)
    } else {
        "5 Alerts".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(title, if alert_count > 0 { theme.crit } else { theme.title }));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 { return; }

    let mut lines: Vec<Line> = Vec::new();
    let avail = inner.height as usize;

    if alerts.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  ", theme.text),
            Span::styled("● ", theme.ok),
            Span::styled("All systems nominal", theme.text_dim),
        ]));
    } else {
        for alert in alerts.iter().take(avail.saturating_sub(1)) {
            let (badge, badge_style) = match alert.severity {
                Severity::Critical => ("CRIT", theme.crit),
                Severity::Warning  => ("WARN", theme.warn),
                Severity::Info     => ("INFO", theme.text_dim),
            };
            lines.push(Line::from(vec![
                Span::styled("  ", theme.text),
                Span::styled(badge, badge_style),
                Span::styled("  ", theme.text),
                Span::styled(alert.prefix(), theme.text_dim),
                Span::styled(alert.message.clone(), theme.text),
            ]));
        }

        if alerts.len() > avail.saturating_sub(1) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  … {} more", alerts.len() - avail.saturating_sub(1)),
                    theme.text_dim,
                ),
            ]));
        }
    }

    // History section — fill remaining lines
    let used = lines.len();
    let remaining = avail.saturating_sub(used);
    if remaining >= 2 && !history.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  ── recent ─────────────────", theme.text_dim),
        ]));
        let hist_lines = remaining.saturating_sub(1);
        for (ts, alert) in history.iter().take(hist_lines) {
            let badge_style = match alert.severity {
                Severity::Critical => theme.crit,
                Severity::Warning  => theme.warn,
                Severity::Info     => theme.text_dim,
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {}", ts), theme.text_dim),
                Span::styled("  ", theme.text),
                Span::styled(alert.severity.label(), badge_style),
                Span::styled("  ", theme.text),
                Span::styled(alert.prefix(), theme.text_dim),
                Span::styled(alert.message.clone(), theme.text_dim),
            ]));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}
