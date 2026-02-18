use crate::models::device::BlockDevice;
use crate::ui::theme::Theme;
use crate::util::human::{fmt_rate, fmt_iops};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline},
    Frame,
};

pub fn render_throughput(
    f: &mut Frame,
    area: Rect,
    devices: &[BlockDevice],
    focused: bool,
    theme: &Theme,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled("2 I/O Throughput", theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Aggregate read + write across all devices
    let total_read:  f64 = devices.iter().map(|d| d.read_bytes_per_sec).sum();
    let total_write: f64 = devices.iter().map(|d| d.write_bytes_per_sec).sum();
    let total_read_iops:  f64 = devices.iter().map(|d| d.read_iops).sum();
    let total_write_iops: f64 = devices.iter().map(|d| d.write_iops).sum();

    // Build aggregate history by summing all devices sample-by-sample
    let sample_count = (inner.width as usize).saturating_sub(4).max(10);
    let read_data:  Vec<u64> = aggregate_history(devices, sample_count, true);
    let write_data: Vec<u64> = aggregate_history(devices, sample_count, false);

    let read_max  = read_data.iter().copied().max().unwrap_or(1).max(1);
    let write_max = write_data.iter().copied().max().unwrap_or(1).max(1);

    // Layout: read label + sparkline, write label + sparkline, IOPS row
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),   // read
            Constraint::Length(2),   // write
            Constraint::Min(1),      // IOPS summary
        ])
        .split(inner);

    // --- READ row ---
    let read_label = Line::from(vec![
        Span::styled("Read  ", theme.read_spark),
        Span::styled(fmt_rate(total_read), theme.text),
    ]);
    f.render_widget(Paragraph::new(read_label), rows[0]);

    let read_spark_area = if rows[0].height >= 2 {
        Rect { x: rows[0].x, y: rows[0].y + 1, width: rows[0].width, height: 1 }
    } else {
        rows[0]
    };

    let read_sparkline = Sparkline::default()
        .data(&read_data)
        .max(read_max)
        .style(theme.read_spark);
    f.render_widget(read_sparkline, read_spark_area);

    // --- WRITE row ---
    let write_label = Line::from(vec![
        Span::styled("Write ", theme.write_spark),
        Span::styled(fmt_rate(total_write), theme.text),
    ]);
    f.render_widget(Paragraph::new(write_label), rows[1]);

    let write_spark_area = if rows[1].height >= 2 {
        Rect { x: rows[1].x, y: rows[1].y + 1, width: rows[1].width, height: 1 }
    } else {
        rows[1]
    };

    let write_sparkline = Sparkline::default()
        .data(&write_data)
        .max(write_max)
        .style(theme.write_spark);
    f.render_widget(write_sparkline, write_spark_area);

    // --- IOPS row ---
    let iops_line = Line::from(vec![
        Span::styled("IOPS  R:", theme.text_dim),
        Span::styled(fmt_iops(total_read_iops), theme.text),
        Span::styled("  W:", theme.text_dim),
        Span::styled(fmt_iops(total_write_iops), theme.text),
    ]);
    f.render_widget(Paragraph::new(iops_line), rows[2]);
}

/// Aggregate per-device history into a single vector of summed KB/s values.
fn aggregate_history(devices: &[BlockDevice], n: usize, read: bool) -> Vec<u64> {
    let mut totals = vec![0u64; n];
    for d in devices {
        let history = if read { &d.read_history } else { &d.write_history };
        let samples = history.last_n(n);
        for (i, &v) in samples.iter().enumerate().take(totals.len()) {
            totals[i] = totals[i].saturating_add(v);
        }
    }
    totals
}
