use crate::models::device::BlockDevice;
use crate::models::filesystem::Filesystem;
use crate::models::smart::{SmartData, SmartStatus};
use crate::ui::theme::Theme;
use crate::util::health_score::{health_score, score_style};
use crate::util::human::{fmt_bytes, fmt_duration_short, fmt_iops, fmt_pct, fmt_rate};
use crate::util::ring_buffer::RingBuffer;
use crate::util::smart_anomaly::{self, DeviceAnomalies};
use crate::util::smart_attr_desc;
use crate::util::smart_baseline::Baseline;
use crate::util::write_endurance::{DeviceEndurance, daily_avg};
use chrono::Local;
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
    filesystems: &[Filesystem],
    scroll: usize,
    history_window: usize,
    smart_test_status: Option<&str>,
    anomalies: Option<&DeviceAnomalies>,
    baseline: Option<&Baseline>,
    endurance: Option<&DeviceEndurance>,
    show_desc: bool,
    theme: &Theme,
) {
    let win_label = WINDOWS[history_window.min(2)].1;
    let name_part = if let Some(alias) = &device.alias {
        format!("{} ({})", device.name, alias)
    } else {
        device.name.clone()
    };
    let title = format!(
        " {} — {}  [w: {} window]",
        name_part,
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
    render_info(f, sections[1], device, filesystems, scroll, smart_test_status, anomalies, baseline, endurance, show_desc, theme);
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

const SPARKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

fn temp_sparkline(rb: &RingBuffer, width: usize) -> (String, u64, u64) {
    let samples = rb.last_n(width);
    if samples.is_empty() { return ("".to_string(), 0, 0); }
    let min = *samples.iter().min().unwrap();
    let max = (*samples.iter().max().unwrap()).max(min + 1);
    let spark: String = samples.iter().map(|&v| {
        SPARKS[(((v - min) * 7 / (max - min)) as usize).min(7)]
    }).collect();
    (spark, min, max)
}

fn render_info(f: &mut Frame, area: Rect, device: &BlockDevice, filesystems: &[Filesystem], scroll: usize, smart_test_status: Option<&str>, anomalies: Option<&DeviceAnomalies>, baseline: Option<&Baseline>, endurance: Option<&DeviceEndurance>, show_desc: bool, theme: &Theme) {
    let mut lines: Vec<Line> = Vec::new();

    // ── Device info ───────────────────────────────────────────────────
    lines.push(section_header("── Device Info ", theme));
    lines.push(kv("Type",      device.dev_type.label().trim(), theme));
    lines.push(kv("Capacity",  &fmt_bytes(device.capacity_bytes), theme));
    if let Some(a) = &device.alias     { lines.push(kv("Alias",     a, theme)); }
    if let Some(s) = &device.serial   { lines.push(kv("Serial",    s, theme)); }
    if let Some(t) = &device.transport { lines.push(kv("Transport", &t.to_uppercase(), theme)); }
    if let Some(sched) = &device.io_scheduler {
        lines.push(kv("I/O Scheduler", sched, theme));
    }

    // SMART poll age
    if let Some(polled_at) = device.smart_polled_at {
        let age_secs = polled_at.elapsed().as_secs();
        lines.push(kv("SMART Last Poll", &fmt_duration_short(age_secs), theme));
    }

    // Health score
    let (hs_str, hs_style) = if device.smart.is_some() {
        let s = health_score(device);
        let style = score_style(s, theme);
        let label = if s >= 80 { format!("{}/100  ✓", s) }
                    else if s >= 50 { format!("{}/100  !", s) }
                    else { format!("{}/100  ✗", s) };
        (label, style)
    } else {
        ("—  (no SMART data yet)".to_string(), theme.text_dim)
    };
    lines.push(kv_colored("Health Score", &hs_str, hs_style, theme));
    lines.push(Line::from(vec![]));

    // ── Write endurance (tracked session data) ────────────────────────
    if let Some(e) = endurance {
        let (daily, days) = daily_avg(e);
        let total_str = fmt_bytes(e.total_bytes_written);
        let daily_str = fmt_bytes(daily as u64);
        let age_str   = if days < 1.0 {
            format!("{:.1}h", days * 24.0)
        } else {
            format!("{:.1}d", days)
        };
        lines.push(section_header("── Write Endurance (tracked) ", theme));
        lines.push(Line::from(vec![
            Span::styled("  Total Written   ", theme.text_dim),
            Span::styled(total_str, theme.text),
            Span::styled(format!("  (over {})", age_str), theme.text_dim),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Daily Avg Write ", theme.text_dim),
            Span::styled(format!("{}/day", daily_str), theme.text),
        ]));
        // SMART LBA-based totals (more accurate for SSDs)
        if let Some(smart) = &device.smart {
            const LBA_SIZE: u64 = 512;
            if let Some(lba_w) = smart.attributes.iter().find(|a| a.id == 241) {
                lines.push(kv("LBAs Written (SMART)", &fmt_bytes(lba_w.raw_value * LBA_SIZE), theme));
            }
            if let Some(lba_r) = smart.attributes.iter().find(|a| a.id == 242) {
                lines.push(kv("LBAs Read (SMART)", &fmt_bytes(lba_r.raw_value * LBA_SIZE), theme));
            }
        }
        lines.push(Line::from(vec![]));
    }

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

            // Wear rate + life projection from power-on hours
            let poh = nvme.power_on_hours;
            if poh > 24 && used_pct > 0 {
                let days_active   = poh as f64 / 24.0;
                let daily_rate    = used_pct as f64 / days_active; // %/day
                let remain_pct    = (100usize.saturating_sub(used_pct)) as f64;
                let days_left     = remain_pct / daily_rate;
                let years_left    = days_left / 365.25;
                let rate_style    = if daily_rate >= 0.1 { theme.warn } else { theme.ok };
                lines.push(Line::from(vec![
                    Span::styled("  Wear Rate       ", theme.text_dim),
                    Span::styled(format!("{:.4}%/day", daily_rate), rate_style),
                    Span::styled(format!("  ({:.0}h POH)", poh), theme.text_dim),
                ]));
                let life_style = if days_left < 180.0 { theme.crit }
                                 else if days_left < 730.0 { theme.warn }
                                 else { theme.ok };
                lines.push(Line::from(vec![
                    Span::styled("  Est Life Left   ", theme.text_dim),
                    Span::styled(
                        format!("~{:.0} days  ({:.1} years)", days_left, years_left),
                        life_style,
                    ),
                ]));
            } else if poh > 0 && used_pct == 0 {
                lines.push(kv("Wear Rate", "< 1%  (minimal use)", theme));
            }
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

    // ── SMART Baseline Δ ──────────────────────────────────────────────
    if let Some(bl) = baseline {
        let hours_elapsed = (Local::now().timestamp() - bl.saved_at) as f64 / 3600.0;
        let age_label = if hours_elapsed < 1.0 {
            format!("{:.0} min ago", hours_elapsed * 60.0)
        } else if hours_elapsed < 48.0 {
            format!("{:.1} h ago", hours_elapsed)
        } else {
            format!("{:.0} d ago", hours_elapsed / 24.0)
        };

        lines.push(section_header("── SMART Baseline Δ ", theme));
        lines.push(Line::from(vec![
            Span::styled("  Saved           ", theme.text_dim),
            Span::styled(format!("{}  ({})", bl.saved_date, age_label), theme.text),
        ]));

        // Power-on hours delta
        if let (Some(bl_poh), Some(curr_poh)) = (
            bl.power_on_hours,
            device.smart.as_ref().and_then(|s| s.power_on_hours),
        ) {
            let delta = curr_poh as i64 - bl_poh as i64;
            lines.push(Line::from(vec![
                Span::styled("  Power On Hrs    ", theme.text_dim),
                Span::styled(
                    format!("{} h → {} h  (Δ {:+})", bl_poh, curr_poh, delta),
                    theme.text,
                ),
            ]));
        }

        // Key count attributes: 5=Reallocated, 197=Pending, 198=Uncorrectable
        for &(id, label) in &[(5u32, "Reallocated"), (197u32, "Pending Scts"), (198u32, "Uncorrectable")] {
            if let Some(curr_attr) = device.smart.as_ref()
                .and_then(|s| s.attributes.iter().find(|a| a.id == id))
            {
                if let Some((base_raw, delta)) = bl.attr_delta(id, curr_attr.raw_value) {
                    let delta_style = if delta > 0 { theme.warn } else { theme.ok };
                    let mut spans = vec![
                        Span::styled(format!("  {:<16}", label), theme.text_dim),
                        Span::styled(
                            format!("base: {}  now: {}  (Δ {:+})", base_raw, curr_attr.raw_value, delta),
                            delta_style,
                        ),
                    ];
                    // Rate-of-change prediction: only meaningful if time elapsed and delta > 0
                    if delta > 0 && hours_elapsed > 0.5 {
                        let rate_per_hr = delta as f64 / hours_elapsed;
                        let proj_30d    = (rate_per_hr * 24.0 * 30.0).round() as i64;
                        spans.push(Span::styled(
                            format!("  → +{:.2}/hr  ~+{} in 30d", rate_per_hr, proj_30d),
                            theme.warn,
                        ));
                    }
                    lines.push(Line::from(spans));
                }
            }
        }

        lines.push(Line::from(vec![
            Span::styled("  B to update baseline", theme.text_dim),
        ]));
        lines.push(Line::from(vec![]));
    } else if device.smart.is_some() {
        lines.push(Line::from(vec![
            Span::styled("  No SMART baseline — press ", theme.text_dim),
            Span::styled("B", theme.ok),
            Span::styled(" to save one", theme.text_dim),
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
            let desc_hint = if show_desc { "D=hide desc" } else { "D=show desc" };
            lines.push(Line::from(vec![
                Span::styled("── SMART Attributes ", theme.text_dim),
                Span::styled("─".repeat(28), theme.text_dim),
                Span::styled(format!("  [{}]", desc_hint), theme.text_dim),
            ]));
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
            // Temperature trend sparkline (if we have history)
            if !device.temp_history.is_empty() {
                let (spark, t_min, t_max) = temp_sparkline(&device.temp_history, 20);
                lines.push(Line::from(vec![
                    Span::styled("  Temp trend   ", theme.text_dim),
                    Span::styled(spark, theme.warn),
                    Span::styled(format!("  min {}°C  max {}°C", t_min, t_max), theme.text_dim),
                ]));
            }
            lines.push(Line::from(vec![]));

            // Column headers
            lines.push(Line::from(vec![
                Span::styled(
                    "  ID   Name                       Type     Val  Wst  Thr  Mgn  Δ  Raw",
                    theme.text_dim,
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(
                    "  ──   ────────────────────────── ──────── ───  ───  ───  ───  ─  ─────────",
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

                // Margin = normalized value - threshold (how far from failure)
                let (mgn_str, mgn_style) = if attr.thresh == 0 {
                    ("  — ".to_string(), theme.text_dim)
                } else {
                    let m = attr.value as i32 - attr.thresh as i32;
                    let style = if m <= 0        { theme.crit }
                                else if m <= 5   { theme.crit }
                                else if m <= 20  { theme.warn }
                                else             { theme.ok };
                    (format!("{:>4}", m), style)
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
                    Span::styled(format!("{}  ", mgn_str), mgn_style),
                    Span::styled(format!("{} ", delta_str), delta_style),
                    Span::styled(format!("{}", attr.raw_str), raw_style),
                ]));
                if show_desc {
                    if let Some(desc) = smart_attr_desc::describe(attr.id) {
                        lines.push(Line::from(vec![
                            Span::styled(format!("       ↳ {}", desc), theme.text_dim),
                        ]));
                    }
                }
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

    // ── SMART anomaly history ─────────────────────────────────────────
    if let Some(dev_anomalies) = anomalies {
        if !dev_anomalies.is_empty() {
            lines.push(section_header("── SMART Anomaly Log ", theme));
            let mut records: Vec<_> = dev_anomalies.values().collect();
            records.sort_by_key(|r| r.first_seen);
            for rec in records {
                let changed = if rec.last_value != rec.first_value {
                    format!("  →  now: {}", rec.last_value)
                } else {
                    String::new()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {:<28}", rec.attr_name), theme.warn),
                    Span::styled(
                        format!("first: {}  val: {}{}", smart_anomaly::fmt_ts(rec.first_seen), rec.first_value, changed),
                        theme.text_dim,
                    ),
                ]));
            }
            lines.push(Line::from(vec![]));
        }
    }

    // ── Partition tree ────────────────────────────────────────────────
    if !device.partitions.is_empty() {
        lines.push(section_header("── Partitions ", theme));
        for (i, part) in device.partitions.iter().enumerate() {
            let is_last  = i == device.partitions.len() - 1;
            let tree     = if is_last { "└─" } else { "├─" };
            let fs       = part.fs_type.as_deref().unwrap_or("?");
            let mnt      = part.mountpoint.as_deref().unwrap_or("");

            // Cross-reference with live filesystem usage
            let live_fs = part.mountpoint.as_deref()
                .and_then(|mp| filesystems.iter().find(|f| f.mount == mp));

            let mut spans = vec![
                Span::styled(format!("  {} ", tree), theme.text_dim),
                Span::styled(format!("{:<12}", part.name), theme.text),
                Span::styled(format!("{:<8}", fs), Style::default().fg(Color::Cyan)),
                Span::styled(format!("{:>10}  ", fmt_bytes(part.size)), theme.text_dim),
                Span::styled(mnt.to_string(), theme.text),
            ];
            if let Some(live) = live_fs {
                let pct   = live.use_pct();
                let style = theme.util_style(pct);
                spans.push(Span::styled(
                    format!("  {}/{} ({:.0}%)",
                        fmt_bytes(live.used_bytes), fmt_bytes(live.total_bytes), pct),
                    style,
                ));
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(vec![]));
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

fn kv_colored(key: &str, val: &str, val_style: ratatui::style::Style, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<18}", key), theme.text_dim),
        Span::styled(val.to_string(), val_style),
    ])
}
