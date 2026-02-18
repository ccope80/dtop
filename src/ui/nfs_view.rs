use crate::app::App;
use crate::collectors::nfs::NfsMountStats;
use crate::util::human::fmt_bytes;
use crate::util::ring_buffer::RingBuffer;
use chrono::Local;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};
use std::collections::HashMap;

const SPARKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Build a 5-char sparkline + RTT label for a single RTT series.
fn rtt_cell(rtt_ms: f64, history: Option<&RingBuffer>) -> String {
    let spark: String = match history {
        None => "     ".to_string(),
        Some(rb) => {
            let samples = rb.last_n(5);
            if samples.is_empty() {
                "     ".to_string()
            } else {
                let max = samples.iter().copied().max().unwrap_or(1).max(1);
                samples.iter().map(|&v| {
                    SPARKS[((v * 7) / max).min(7) as usize]
                }).collect()
            }
        }
    };
    if rtt_ms == 0.0 {
        format!("{} —    ", spark)
    } else if rtt_ms < 10.0 {
        format!("{} {:.1}ms", spark, rtt_ms)
    } else {
        format!("{} {:.0}ms ", spark, rtt_ms)
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let area  = f.area();
    let theme = app.theme.clone();

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Header
    let now = Local::now().format("%H:%M:%S").to_string();
    let title = format!(" DTop — NFS / Network Mounts   {}", now);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(title, theme.title))).style(theme.header),
        root[0],
    );

    // Body
    let body = root[1];
    if app.nfs_mounts.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(vec![]),
            Line::from(vec![Span::styled("  No NFS or network mounts detected on this system.", theme.text_dim)]),
            Line::from(vec![]),
            Line::from(vec![Span::styled("  NFS mounts appear automatically when you mount a remote filesystem:", theme.text_dim)]),
            Line::from(vec![Span::styled("    mount -t nfs4 server:/export /mnt/point", theme.text_dim)]),
            Line::from(vec![]),
            Line::from(vec![Span::styled("  Statistics are sourced from /proc/self/mountstats.", theme.text_dim)]),
        ])
        .block(Block::default().borders(Borders::ALL).border_style(theme.border)
            .title(Span::styled("Network Mounts", theme.title)));
        f.render_widget(msg, body);
    } else {
        let rows_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)])
            .split(body);

        render_nfs_table(f, rows_area[0], &app.nfs_mounts, &app.nfs_rtt_history, &theme);
    }

    // Footer
    let footer_spans = vec![
        Span::styled(" ", theme.footer_bg),
        Span::styled(" Esc ", theme.footer_key), Span::styled("Dashboard  ", theme.footer_text),
        Span::styled(" ↑↓ ", theme.footer_key),  Span::styled("Scroll  ", theme.footer_text),
        Span::styled(" q ",  theme.footer_key),  Span::styled("Quit  ", theme.footer_text),
    ];
    f.render_widget(
        Paragraph::new(Line::from(footer_spans)).style(theme.footer_bg),
        root[2],
    );
}

fn render_nfs_table(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    mounts: &[NfsMountStats],
    rtt_history: &HashMap<String, (RingBuffer, RingBuffer)>,
    theme: &crate::ui::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled(
            format!("Network Mounts  ({} mounted)", mounts.len()),
            theme.title,
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let header = Row::new(vec![
        Cell::from("Mount").style(theme.text_dim),
        Cell::from("Type").style(theme.text_dim),
        Cell::from("Server").style(theme.text_dim),
        Cell::from("Age").style(theme.text_dim),
        Cell::from("R-Hist  RTT").style(theme.text_dim),
        Cell::from("W-Hist  RTT").style(theme.text_dim),
        Cell::from("Status").style(theme.text_dim),
        Cell::from("Read").style(theme.text_dim),
        Cell::from("Written").style(theme.text_dim),
    ])
    .height(1);

    let rows: Vec<Row> = mounts.iter().map(|m| {
        let status = m.status_str();
        let status_style = match status {
            "OK"       => theme.ok,
            "SLOW"     => theme.warn,
            "DEGRADED" => theme.crit,
            _          => theme.text_dim,
        };

        let age_str = if m.age_secs < 3600 {
            format!("{}m", m.age_secs / 60)
        } else {
            format!("{}h", m.age_secs / 3600)
        };

        // Truncate server to reasonable length
        let server = if m.device.len() > 28 {
            format!("{}…", &m.device[..27])
        } else {
            m.device.clone()
        };

        let hist = rtt_history.get(&m.mount);
        Row::new(vec![
            Cell::from(m.mount.clone()).style(theme.text),
            Cell::from(m.fstype.clone()).style(theme.text_dim),
            Cell::from(server).style(theme.text_dim),
            Cell::from(age_str).style(theme.text_dim),
            Cell::from(rtt_cell(m.read_rtt_ms,  hist.map(|p| &p.0))).style(rtt_style(m.read_rtt_ms, theme)),
            Cell::from(rtt_cell(m.write_rtt_ms, hist.map(|p| &p.1))).style(rtt_style(m.write_rtt_ms, theme)),
            Cell::from(status).style(status_style),
            Cell::from(fmt_bytes(m.server_bytes_read)).style(theme.read_spark),
            Cell::from(fmt_bytes(m.server_bytes_written)).style(theme.write_spark),
        ])
    }).collect();

    let widths = [
        Constraint::Min(16),
        Constraint::Length(6),
        Constraint::Min(18),
        Constraint::Length(5),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(9),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .row_highlight_style(Style::default());

    f.render_widget(table, inner);
}

fn rtt_style(rtt: f64, theme: &crate::ui::theme::Theme) -> ratatui::style::Style {
    if rtt == 0.0   { theme.text_dim }
    else if rtt < 5.0  { theme.ok }
    else if rtt < 50.0 { theme.warn }
    else               { theme.crit }
}
