use crate::alerts::{Alert, Severity};
use crate::ui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::collections::{HashSet, VecDeque};

pub fn render_alerts_panel(
    f: &mut Frame,
    area: Rect,
    alerts: &[Alert],
    history: &VecDeque<(String, Alert)>,
    acked: &HashSet<String>,
    focused: bool,
    theme: &Theme,
    state: &mut ListState,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let unacked = alerts.iter().filter(|a| !acked.contains(&a.key())).count();
    let title = if unacked > 0 {
        format!("5 Alerts  ({} active)", unacked)
    } else if !alerts.is_empty() {
        format!("5 Alerts  ({} acked)", alerts.len())
    } else {
        "5 Alerts".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(title, if unacked > 0 { theme.crit } else { theme.title }));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 { return; }

    let avail = inner.height as usize;

    if alerts.is_empty() {
        let lines = vec![Line::from(vec![
            Span::styled("  ", theme.text),
            Span::styled("● ", theme.ok),
            Span::styled("All systems nominal", theme.text_dim),
        ])];
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }

    // How many alert rows can we show (leave 1 row for "…more" if needed)
    let max_alert_rows = avail.saturating_sub(1);
    let shown_alerts = alerts.len().min(max_alert_rows);
    let has_overflow = alerts.len() > shown_alerts;

    // Build alert list items (including overflow as a plain item)
    let mut items: Vec<ListItem> = alerts.iter().take(shown_alerts).map(|alert| {
        let is_acked = acked.contains(&alert.key());
        let (badge, badge_style) = match alert.severity {
            Severity::Critical => ("CRIT", if is_acked { theme.text_dim } else { theme.crit }),
            Severity::Warning  => ("WARN", if is_acked { theme.text_dim } else { theme.warn }),
            Severity::Info     => ("INFO", theme.text_dim),
        };
        let msg_style = if is_acked { theme.text_dim } else { theme.text };
        let ack_mark  = if is_acked { " [ack]" } else { "" };
        ListItem::new(Line::from(vec![
            Span::styled("  ", theme.text),
            Span::styled(badge, badge_style),
            Span::styled("  ", theme.text),
            Span::styled(alert.prefix(), theme.text_dim),
            Span::styled(format!("{}{}", alert.message, ack_mark), msg_style),
        ]))
    }).collect();

    if has_overflow {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                format!("  … {} more", alerts.len() - shown_alerts),
                theme.text_dim,
            ),
        ])));
    }

    let list_rows = items.len();

    // Remaining rows after the alert list
    let remaining = avail.saturating_sub(list_rows);
    let has_history = remaining >= 2 && !history.is_empty();
    let hist_height = if has_history { remaining as u16 } else { 0 };
    let list_height = (list_rows as u16).min(inner.height.saturating_sub(hist_height));

    // Split inner area: top = alert list, bottom = history
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(list_height),
            Constraint::Length(hist_height),
        ])
        .split(inner);

    // Render the alert List with stateful highlight
    let alert_list = List::new(items)
        .highlight_style(if focused { theme.selected } else { theme.text });
    f.render_stateful_widget(alert_list, chunks[0], state);

    // Render history section
    if has_history {
        let mut hist_lines: Vec<Line> = Vec::new();
        hist_lines.push(Line::from(vec![
            Span::styled("  ── recent ─────────────────", theme.text_dim),
        ]));
        let hist_entry_lines = (hist_height as usize).saturating_sub(1);
        for (ts, alert) in history.iter().take(hist_entry_lines) {
            let badge_style = match alert.severity {
                Severity::Critical => theme.crit,
                Severity::Warning  => theme.warn,
                Severity::Info     => theme.text_dim,
            };
            hist_lines.push(Line::from(vec![
                Span::styled(format!("  {}", ts), theme.text_dim),
                Span::styled("  ", theme.text),
                Span::styled(alert.severity.label(), badge_style),
                Span::styled("  ", theme.text),
                Span::styled(alert.prefix(), theme.text_dim),
                Span::styled(alert.message.clone(), theme.text_dim),
            ]));
        }
        f.render_widget(Paragraph::new(hist_lines), chunks[1]);
    }
}
