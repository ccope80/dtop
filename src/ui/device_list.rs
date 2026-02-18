use crate::models::device::{BlockDevice, DeviceType};
use crate::models::smart::SmartStatus;
use crate::ui::theme::Theme;
use crate::util::human::fmt_rate;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub fn render_device_list(
    f: &mut Frame,
    area: Rect,
    devices: &[BlockDevice],
    state: &mut ListState,
    focused: bool,
    theme: &Theme,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let items: Vec<ListItem> = devices
        .iter()
        .map(|d| device_row(d, theme))
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled("1 Devices", theme.title));

    let list = List::new(items)
        .block(block)
        .highlight_style(theme.selected)
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, state);
}

fn device_row(d: &BlockDevice, theme: &Theme) -> ListItem<'static> {
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

    let spans = vec![
        Span::styled(format!("  {:<7}", d.name), theme.text),
        Span::styled(format!("{} ", d.dev_type.label()), type_colour(d, theme)),
        Span::styled(dot.to_string(), dot_style),
        Span::styled(" ".to_string(), theme.text),
        Span::styled(temp_str, temp_style),
        Span::styled("  ".to_string(), theme.text),
        Span::styled(util_bar, util_style),
        Span::styled(util_pct, util_style),
        Span::styled(format!("  R:{:>9}  W:{:>9}", read_s, write_s), theme.text_dim),
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
