use crate::models::device::BlockDevice;
use crate::models::smart::{SmartData, SmartStatus};
use crate::ui::theme::Theme;
use crate::util::human::{fmt_bytes, fmt_iops, fmt_pct, fmt_rate};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline, Wrap},
    Frame,
};

// History window constants
const WINDOWS: [(usize, &str); 3] = [
    (30,   "60s"),
    (150,  " 5m"),
    (1800, " 1h"),
];

pub fn render_detail(
    f: &mut Frame,
    area: Rect,
    device: &BlockDevice,
    scroll: usize,
    history_window: usize,
    smart_test_status: Option<&str>,
    theme: &Theme,
) {
    let win_label = WINDOWS[history_window.min(2)].1;
    let title = format!(
        " {} — {}  [w: {} window]",
        device.name,
        device.model.as_deref().unwrap_or("Unknown model"),
        win_label
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled(title, theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),   // sparklines + temp sparkline + latency
            Constraint::Min(4),      // scrollable info
        ])
        .split(inner);

    render_sparklines(f, sections[0], device, history_window, theme);
    render_info(f, sections[1], device, scroll, smart_test_status, theme);
}

fn render_sparklines(f: &mut Frame, area: Rect, device: &BlockDevice, history_window: usize, theme: &Theme) {
    let n_samples = WINDOWS[history_window.min(2)].0;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // read label
            Constraint::Length(1),  // read sparkline
            Constraint::Length(1),  // write label
            Constraint::Length(1),  // write sparkline
            Constraint::Length(1),  // temp label
            Constraint::Length(1),  // temp sparkline
            Constraint::Length(1),  // latency + util line
            Constraint::Min(0),
        ])
        .split(area);

    let n = (area.width as usize).saturating_sub(2).max(4);
    // Use the min of n (display width) and n_samples (time window)
    let samples = n_samples.min(n * 10).max(4); // fetch more than needed; sparkline uses last n
    let read_data  = device.read_history .last_n(samples);
    let write_data = device.write_history.last_n(samples);
    let read_max   = read_data .iter().copied().max().unwrap_or(1).max(1);
    let write_max  = write_data.iter().copied().max().unwrap_or(1).max(1);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Read  ", theme.read_spark),
            Span::styled(fmt_rate(device.read_bytes_per_sec), theme.text),
            Span::styled(format!("   IOPS: {}", fmt_iops(device.read_iops)), theme.text_dim),
        ])),
        rows[0],
    );
    f.render_widget(
        Sparkline::default().data(&read_data).max(read_max).style(theme.read_spark),
        rows[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Write ", theme.write_spark),
            Span::styled(fmt_rate(device.write_bytes_per_sec), theme.text),
            Span::styled(format!("   IOPS: {}", fmt_iops(device.write_iops)), theme.text_dim),
        ])),
        rows[2],
    );
    f.render_widget(
        Sparkline::default().data(&write_data).max(write_max).style(theme.write_spark),
        rows[3],
    );

    // Temperature label + sparkline
    let temp_data = device.temp_history.last_n(samples);
    let temp_max  = temp_data.iter().copied().max().unwrap_or(1).max(1);
    let temp_str  = match device.temperature() {
        Some(t) => format!("{}°C", t),
        None    => "N/A".to_string(),
    };
    let temp_style = match device.temperature() {
        Some(t) if (device.rotational && t >= 60) || (!device.rotational && t >= 70) => theme.crit,
        Some(t) if (device.rotational && t >= 50) || (!device.rotational && t >= 55) => theme.warn,
        Some(_) => theme.ok,
        None    => theme.text_dim,
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Temp  ", theme.text_dim),
            Span::styled(temp_str, temp_style),
        ])),
        rows[4],
    );
    f.render_widget(
        Sparkline::default().data(&temp_data).max(temp_max).style(temp_style),
        rows[5],
    );

    // Latency + util row
    let r_lat = device.avg_read_latency_ms;
    let w_lat = device.avg_write_latency_ms;
    let lat_line = Line::from(vec![
        Span::styled("Latency ", theme.text_dim),
        Span::styled("R:", theme.read_spark),
        Span::styled(fmt_latency(r_lat), lat_style(r_lat, theme)),
        Span::styled("  W:", theme.write_spark),
        Span::styled(fmt_latency(w_lat), lat_style(w_lat, theme)),
        Span::styled("   Util:", theme.text_dim),
        Span::styled(fmt_pct(device.io_util_pct), theme.util_style(device.io_util_pct)),
    ]);
    f.render_widget(Paragraph::new(lat_line), rows[6]);
}

fn fmt_latency(ms: f64) -> String {
    if ms <= 0.0       { "    —".to_string() }
    else if ms < 1.0   { format!("{:.2}ms", ms) }
    else if ms < 10.0  { format!("{:.1}ms", ms) }
    else               { format!("{:.0}ms", ms) }
}

fn lat_style(ms: f64, theme: &Theme) -> Style {
    if ms <= 0.0       { theme.text_dim }
    else if ms < 5.0   { theme.ok }
    else if ms < 20.0  { theme.warn }
    else               { theme.crit }
}

fn render_info(f: &mut Frame, area: Rect, device: &BlockDevice, scroll: usize, smart_test_status: Option<&str>, theme: &Theme) {
    let mut lines: Vec<Line> = Vec::new();

    // ── Device info ───────────────────────────────────────────────────
    lines.push(section_header("── Device Info ", theme));
    lines.push(kv("Type",      device.dev_type.label().trim(), theme));
    lines.push(kv("Capacity",  &fmt_bytes(device.capacity_bytes), theme));
    if let Some(s) = &device.serial   { lines.push(kv("Serial",    s, theme)); }
    if let Some(t) = &device.transport { lines.push(kv("Transport", &t.to_uppercase(), theme)); }
    lines.push(Line::from(vec![]));

    // ── Drive endurance / lifespan estimate ───────────────────────────
    if let Some(smart) = &device.smart {
        if let Some(nvme) = &smart.nvme {
            // NVMe: percentage_used from SMART health log
            let used_pct = nvme.percentage_used as usize;
            let remain   = 100usize.saturating_sub(used_pct);
            let bar_used = (used_pct * 20 / 100).min(20);
            let bar_free = 20 - bar_used;
            let endurance_style = if used_pct >= 90 { theme.crit }
                                  else if used_pct >= 70 { theme.warn }
                                  else { theme.ok };
            lines.push(section_header("── NVMe Endurance ", theme));
            lines.push(Line::from(vec![
                Span::styled("  Endurance Used  ", theme.text_dim),
                Span::styled("█".repeat(bar_used), endurance_style),
                Span::styled("░".repeat(bar_free), theme.text_dim),
                Span::styled(format!("  {}% used, {}% remaining", used_pct, remain), endurance_style),
            ]));
            lines.push(kv("Data Written", &fmt_bytes(nvme.bytes_written()), theme));
            lines.push(Line::from(vec![]));
        } else if let Some(poh) = smart.power_on_hours {
            // HDD/SSD: power-on hours vs ~50k hour lifespan estimate
            const LIFESPAN_H: u64 = 50_000;
            let pct = ((poh * 100) / LIFESPAN_H).min(100) as usize;
            let remain_h = LIFESPAN_H.saturating_sub(poh);
            let bar_used = (pct * 20 / 100).min(20);
            let bar_free = 20 - bar_used;
            let life_style = if pct >= 90 { theme.crit }
                             else if pct >= 70 { theme.warn }
                             else { theme.ok };
            lines.push(section_header("── Estimated Lifespan ", theme));
            lines.push(Line::from(vec![
                Span::styled("  Life Consumed   ", theme.text_dim),
                Span::styled("█".repeat(bar_used), life_style),
                Span::styled("░".repeat(bar_free), theme.text_dim),
                Span::styled(format!("  {}h / ~{}kh  ({} h remaining)", poh, LIFESPAN_H / 1000, remain_h), theme.text_dim),
            ]));
            lines.push(Line::from(vec![]));
        }
    }

    // ── SMART test status ─────────────────────────────────────────────
    if let Some(status) = smart_test_status {
        lines.push(Line::from(vec![
            Span::styled("  SMART test: ", theme.text_dim),
            Span::styled(status.to_string(), theme.ok),
        ]));
        lines.push(Line::from(vec![]));
    }

    // ── SMART / NVMe ──────────────────────────────────────────────────
    if let Some(smart) = &device.smart {
        if let Some(nvme) = &smart.nvme {
            lines.push(section_header("── NVMe Health Log ", theme));
            lines.push(kv("Status",          smart.status.label().trim(), theme));
            lines.push(kv("Temperature",     &format!("{}°C", nvme.temperature_celsius), theme));
            lines.push(kv("Percentage Used", &format!("{}%", nvme.percentage_used), theme));
            lines.push(kv("Available Spare",
                &format!("{}%  (threshold: {}%)", nvme.available_spare_pct, nvme.available_spare_threshold),
                theme));
            lines.push(kv("Power On Hours",   &format!("{} h", nvme.power_on_hours), theme));
            lines.push(kv("Unsafe Shutdowns", &nvme.unsafe_shutdowns.to_string(), theme));
            lines.push(kv("Media Errors",     &nvme.media_errors.to_string(), theme));
            lines.push(kv("Error Log Entries",&nvme.error_log_entries.to_string(), theme));
            lines.push(kv("Data Read",        &fmt_bytes(nvme.bytes_read()), theme));
            lines.push(kv("Data Written",     &fmt_bytes(nvme.bytes_written()), theme));
            lines.push(Line::from(vec![]));
        } else {
            lines.push(section_header("── SMART Attributes ", theme));
            let (health_str, health_style) = match smart.status {
                SmartStatus::Passed  => ("PASSED", theme.ok),
                SmartStatus::Warning => ("WARNING — pre-fail attr at risk", theme.warn),
                SmartStatus::Failed  => ("FAILED", theme.crit),
                SmartStatus::Unknown => ("Unknown", theme.text_dim),
            };
            lines.push(Line::from(vec![
                Span::styled("  Health: ", theme.text_dim),
                Span::styled(health_str, health_style),
            ]));
            if let Some(poh) = smart.power_on_hours {
                lines.push(kv("Power On Hours", &format!("{} h", poh), theme));
            }
            if let Some(temp) = smart.temperature {
                lines.push(kv("Temperature", &format!("{}°C", temp), theme));
            }
            lines.push(Line::from(vec![]));

            // Column headers — added Δ column
            lines.push(Line::from(vec![
                Span::styled(
                    "  ID   Name                       Type     Val  Wst  Thr  Δ  Raw",
                    theme.text_dim,
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(
                    "  ──   ────────────────────────── ──────── ───  ───  ───  ─  ─────────",
                    theme.text_dim,
                ),
            ]));

            for attr in &smart.attributes {
                let type_str   = if attr.prefail { "Pre-fail" } else { "Old_age " };
                let row_style  = if attr.is_at_risk() { theme.warn } else { theme.text };
                let raw_style  = if attr.prefail && attr.thresh > 0 && attr.value <= attr.thresh {
                    theme.crit
                } else {
                    row_style
                };

                // Delta vs previous poll
                let (delta_str, delta_style) = delta_arrow(attr.id, attr.value, device.smart_prev.as_ref(), theme);

                lines.push(Line::from(vec![
                    Span::styled(format!("  {:>3}  ", attr.id), theme.text_dim),
                    Span::styled(format!("{:<26}  ", attr.name), row_style),
                    Span::styled(format!("{:<8}  ", type_str), theme.text_dim),
                    Span::styled(format!("{:>3}  ", attr.value), row_style),
                    Span::styled(format!("{:>3}  ", attr.worst), theme.text_dim),
                    Span::styled(format!("{:>3}  ", attr.thresh), theme.text_dim),
                    Span::styled(format!("{} ", delta_str), delta_style),
                    Span::styled(format!("{}", attr.raw_str), raw_style),
                ]));
            }

            if smart.attributes.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("  No ATA attributes available", theme.text_dim),
                ]));
            }
            lines.push(Line::from(vec![]));
        }
    } else {
        lines.push(section_header("── SMART ", theme));
        lines.push(Line::from(vec![
            Span::styled("  Polling… (smartctl runs in background)", theme.text_dim),
        ]));
        lines.push(Line::from(vec![]));
    }

    // ── Partition tree ────────────────────────────────────────────────
    if !device.partitions.is_empty() {
        lines.push(section_header("── Partitions ", theme));
        for (i, part) in device.partitions.iter().enumerate() {
            let is_last  = i == device.partitions.len() - 1;
            let tree     = if is_last { "└─" } else { "├─" };
            let fs       = part.fs_type.as_deref().unwrap_or("?");
            let mnt      = part.mountpoint.as_deref().unwrap_or("");
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", tree), theme.text_dim),
                Span::styled(format!("{:<12}", part.name), theme.text),
                Span::styled(format!("{:<8}", fs), Style::default().fg(Color::Cyan)),
                Span::styled(format!("{:>10}  ", fmt_bytes(part.size)), theme.text_dim),
                Span::styled(mnt.to_string(), theme.text),
            ]));
        }
    }

    let para = Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

/// Compare `curr_value` for `attr_id` against `smart_prev`, return (arrow, style).
fn delta_arrow(
    attr_id: u32,
    curr_value: u16,
    prev: Option<&SmartData>,
    theme: &Theme,
) -> (String, Style) {
    let Some(prev_smart) = prev else {
        return (" ".to_string(), theme.text_dim);
    };
    let Some(prev_attr) = prev_smart.attributes.iter().find(|a| a.id == attr_id) else {
        return (" ".to_string(), theme.text_dim);
    };

    use std::cmp::Ordering::*;
    match curr_value.cmp(&prev_attr.value) {
        Less    => ("↓".to_string(), theme.crit),   // value dropped → bad
        Greater => ("↑".to_string(), theme.ok),     // value rose → recovered
        Equal   => ("·".to_string(), theme.text_dim),
    }
}

fn section_header(title: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(title.to_string(), theme.text_dim),
        Span::styled("─".repeat(40), theme.text_dim),
    ])
}

fn kv(key: &str, val: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<18}", key), theme.text_dim),
        Span::styled(val.to_string(), theme.text),
    ])
}
