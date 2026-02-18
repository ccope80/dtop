use crate::app::App;
use crate::ui::theme::Theme;
use crate::util::human::fmt_rate;
use chrono::Local;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let area  = f.area();
    let theme = app.theme.clone();

    // Root: header | body | footer
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Header
    let now = Local::now().format("%H:%M:%S").to_string();
    let title = format!(
        " DTop — Process I/O   Sorted: {}   {}",
        app.process_sort.label(),
        now
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(title, theme.title))).style(theme.header),
        root[0],
    );

    let body = root[1];
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(6)])
        .split(body);

    render_process_table(f, rows[0], app, &theme);
    render_bottom_bar(f, rows[1], app, &theme);

    // Footer
    render_proc_footer(f, root[2], &theme);
}

fn render_process_table(f: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled("I/O by Process", theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let header_cells = ["PID", "USER", "READ/s", "WRITE/s", "COMMAND"]
        .iter()
        .map(|h| Cell::from(*h).style(theme.text_dim));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1);

    let procs = &app.process_io;

    let total_read:  f64 = procs.iter().map(|p| p.read_per_sec).sum();
    let total_write: f64 = procs.iter().map(|p| p.write_per_sec).sum();

    let mut rows_data: Vec<Row> = procs
        .iter()
        .map(|p| {
            let write_style = theme.util_style(
                (p.write_per_sec / (total_write + 1.0).max(1.0) * 100.0).min(100.0)
            );
            Row::new(vec![
                Cell::from(p.pid.to_string()).style(theme.text_dim),
                Cell::from(format!("{:<8}", &p.username[..p.username.len().min(8)])).style(theme.text_dim),
                Cell::from(fmt_rate(p.read_per_sec)).style(theme.read_spark),
                Cell::from(fmt_rate(p.write_per_sec)).style(write_style),
                Cell::from(p.comm.clone()).style(theme.text),
            ])
        })
        .collect();

    // Totals row
    if !procs.is_empty() {
        rows_data.push(Row::new(vec![
            Cell::from(""),
            Cell::from("Totals").style(theme.text_dim),
            Cell::from(fmt_rate(total_read)).style(theme.read_spark),
            Cell::from(fmt_rate(total_write)).style(theme.write_spark),
            Cell::from(""),
        ]));
    } else {
        rows_data.push(Row::new(vec![
            Cell::from(""),
            Cell::from(""),
            Cell::from("  No I/O activity").style(theme.text_dim),
            Cell::from(""),
            Cell::from(""),
        ]));
    }

    let widths = [
        Constraint::Length(7),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Min(10),
    ];

    let table = Table::new(rows_data, widths)
        .header(header)
        .column_spacing(1)
        .row_highlight_style(theme.selected);

    f.render_stateful_widget(table, inner, &mut app.process_table_state);
}

fn render_bottom_bar(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: aggregate sparklines
    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("Aggregate I/O", theme.title));
    let left_inner = left_block.inner(cols[0]);
    f.render_widget(left_block, cols[0]);

    let spark_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), Constraint::Length(1),
            Constraint::Length(1), Constraint::Length(1),
        ])
        .split(left_inner);

    let total_read:  f64 = app.process_io.iter().map(|p| p.read_per_sec).sum();
    let total_write: f64 = app.process_io.iter().map(|p| p.write_per_sec).sum();
    let n = (left_inner.width as usize).saturating_sub(2).max(4);
    let read_hist  = app.proc_read_history .last_n(n);
    let write_hist = app.proc_write_history.last_n(n);
    let rmax = read_hist .iter().copied().max().unwrap_or(1).max(1);
    let wmax = write_hist.iter().copied().max().unwrap_or(1).max(1);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Read  ", theme.read_spark),
            Span::styled(fmt_rate(total_read), theme.text),
        ])),
        spark_rows[0],
    );
    f.render_widget(
        Sparkline::default().data(&read_hist).max(rmax).style(theme.read_spark),
        spark_rows[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Write ", theme.write_spark),
            Span::styled(fmt_rate(total_write), theme.text),
        ])),
        spark_rows[2],
    );
    f.render_widget(
        Sparkline::default().data(&write_hist).max(wmax).style(theme.write_spark),
        spark_rows[3],
    );

    // Right: per-device load bars
    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("Device Load", theme.title));
    let right_inner = right_block.inner(cols[1]);
    f.render_widget(right_block, cols[1]);

    let mut lines: Vec<Line> = Vec::new();
    for dev in app.devices.iter().take(right_inner.height as usize) {
        let filled = ((dev.io_util_pct / 100.0) * 10.0).round() as usize;
        let filled = filled.min(10);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
        let style = theme.util_style(dev.io_util_pct);
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<7}", dev.name), theme.text),
            Span::styled(bar, style),
            Span::styled(format!(" {:>3.0}%", dev.io_util_pct), style),
        ]));
    }
    f.render_widget(Paragraph::new(lines), right_inner);
}

fn render_proc_footer(f: &mut Frame, area: Rect, theme: &Theme) {
    let spans = vec![
        Span::styled(" ", theme.footer_bg),
        Span::styled(" Esc ", theme.footer_key),  Span::styled("Dashboard  ", theme.footer_text),
        Span::styled(" s ", theme.footer_key),    Span::styled("Cycle Sort  ", theme.footer_text),
        Span::styled(" ↑↓ ", theme.footer_key),   Span::styled("Select  ", theme.footer_text),
        Span::styled(" q ", theme.footer_key),    Span::styled("Quit  ", theme.footer_text),
    ];
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(theme.footer_bg),
        area,
    );
}
