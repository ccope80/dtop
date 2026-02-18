use crate::models::device::{BlockDevice, DeviceType};
use crate::models::smart::SmartStatus;
use crate::ui::theme::Theme;
use crate::util::health_score::{health_score, score_style, score_str};
use crate::util::human::fmt_rate;
use crate::util::ring_buffer::RingBuffer;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use std::collections::HashMap;

fn filter_active(d: &BlockDevice, filter_label: &str) -> bool {
    match filter_label {
        "NVMe" => d.dev_type == DeviceType::NVMe,
        "SSD"  => d.dev_type == DeviceType::SSD,
        "HDD"  => d.dev_type == DeviceType::HDD,
        _      => true,
    }
}

pub fn render_device_list(
    f: &mut Frame,
    area: Rect,
    devices: &[BlockDevice],
    state: &mut ListState,
    focused: bool,
    filter_label: &str,
    sort_label: &str,
    health_history: &HashMap<String, Vec<u8>>,
    io_history: &HashMap<String, (RingBuffer, RingBuffer)>,
    theme: &Theme,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let items: Vec<ListItem> = devices
        .iter()
        .map(|d| {
            let hist    = health_history.get(&d.name).map(|v| v.as_slice());
            let io_hist = io_history.get(&d.name);
            device_row(d, filter_active(d, filter_label), hist, io_hist, theme)
        })
        .collect();

    // Build a compact title showing active filter and sort modifiers
    let active_count = if filter_label == "All" {
        devices.len()
    } else {
        devices.iter().filter(|d| filter_active(d, filter_label)).count()
    };
    let count_word = if filter_label == "All" { "total" } else { "shown" };
    let mut title = format!("1 Devices  ({} {})", active_count, count_word);
    if filter_label != "All" {
        title = format!("1 Devices  [f:{}]  ({} shown)", filter_label, active_count);
    }
    if sort_label != "Natural" {
        title = format!("{}  [s:{}]", title, sort_label);
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(title, theme.title));

    let list = List::new(items)
        .block(block)
        .highlight_style(theme.selected)
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, state);
}

const SPARKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

fn io_spark(hist: Option<&RingBuffer>) -> String {
    match hist {
        None => "    ".to_string(),
        Some(rb) => {
            let samples = rb.last_n(4);
            if samples.is_empty() {
                "    ".to_string()
            } else {
                let max = samples.iter().copied().max().unwrap_or(1).max(1);
                samples.iter().map(|&v| SPARKS[((v * 7) / max).min(7) as usize]).collect()
            }
        }
    }
}

fn health_spark(hist: Option<&[u8]>) -> String {
    match hist {
        None | Some([]) => "     ".to_string(),
        Some(v) => {
            let n = v.len().min(5);
            v[v.len() - n..].iter().map(|&s| {
                SPARKS[((s as usize) * 7 / 100).min(7)]
            }).collect::<String>()
             + &" ".repeat(5 - n)
        }
    }
}

fn device_row(d: &BlockDevice, active: bool, hist: Option<&[u8]>, io_hist: Option<&(RingBuffer, RingBuffer)>, theme: &Theme) -> ListItem<'static> {
    // When this device doesn't match the current filter, dim the entire row.
    if !active {
        let spans = vec![
            Span::styled(format!("  {:<7}", d.name), theme.text_dim),
            Span::styled(d.dev_type.label().to_string(), theme.text_dim),
            Span::styled("   ·  ".to_string(), theme.text_dim),
            Span::styled("     ".to_string(), theme.text_dim),
            Span::styled("   ---  ".to_string(), theme.text_dim),
            Span::styled("░░░░░░░░".to_string(), theme.text_dim),
            Span::styled("  -%".to_string(), theme.text_dim),
        ];
        return ListItem::new(Line::from(spans));
    }

    // Health score badge (3 chars + space)
    let s = health_score(d);
    let hs_str   = score_str(d);
    let hs_style = score_style(s, theme);

    // Health indicator dot
    let (dot, dot_style) = match d.smart_status() {
        SmartStatus::Unknown => ("·", theme.text_dim),
        SmartStatus::Passed  => ("●", theme.ok),
        SmartStatus::Warning => ("●", theme.warn),
        SmartStatus::Failed  => ("●", theme.crit),
    };

    // Temperature
    let temp_str = match d.temperature() {
        Some(t) => format!("{:>3}°C", t),
        None    => " N/A".to_string(),
    };
    let temp_style = match d.temperature() {
        Some(t) if (d.rotational && t >= 60) || (!d.rotational && t >= 70) => theme.crit,
        Some(t) if (d.rotational && t >= 50) || (!d.rotational && t >= 55) => theme.warn,
        Some(_) => theme.text,
        None    => theme.text_dim,
    };

    // I/O utilisation bar (8 chars)
    let util_bar = util_bar(d.io_util_pct);
    let util_pct = format!("{:>3.0}%", d.io_util_pct);
    let util_style = theme.util_style(d.io_util_pct);

    let read_s  = fmt_rate(d.read_bytes_per_sec);
    let write_s = fmt_rate(d.write_bytes_per_sec);

    let spark = health_spark(hist);
    let spark_style = score_style(health_score(d), theme);

    let rspk = io_spark(io_hist.map(|p| &p.0));
    let wspk = io_spark(io_hist.map(|p| &p.1));

    let spans = vec![
        Span::styled(format!("  {:<7}", d.name), theme.text),
        Span::styled(d.dev_type.label().to_string(), type_colour(d, theme)),
        Span::styled(hs_str, hs_style),
        Span::styled(dot.to_string(), dot_style),
        Span::styled(" ".to_string(), theme.text),
        Span::styled(spark, spark_style),
        Span::styled(" ".to_string(), theme.text),
        Span::styled(temp_str, temp_style),
        Span::styled("  ".to_string(), theme.text),
        Span::styled(util_bar, util_style),
        Span::styled(util_pct, util_style),
        Span::styled("  R".to_string(), theme.text_dim),
        Span::styled(rspk, theme.read_spark),
        Span::styled(format!(":{:>9}  W", read_s), theme.text_dim),
        Span::styled(wspk, theme.write_spark),
        Span::styled(format!(":{:>9}", write_s), theme.text_dim),
    ];

    ListItem::new(Line::from(spans))
}

fn util_bar(pct: f64) -> String {
    let filled = ((pct / 100.0) * 8.0).round() as usize;
    let filled = filled.min(8);
    let empty  = 8 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn type_colour(d: &BlockDevice, theme: &Theme) -> Style {
    match d.dev_type {
        DeviceType::NVMe    => Style::default().fg(Color::Magenta),
        DeviceType::SSD     => Style::default().fg(Color::Cyan),
        DeviceType::HDD     => Style::default().fg(Color::Blue),
        DeviceType::Virtual => Style::default().fg(Color::DarkGray),
        DeviceType::Unknown => theme.text_dim,
    }
}
