use crate::models::device::{BlockDevice, DeviceType};
use crate::models::smart::SmartStatus;
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_smart_panel(
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
        .title(Span::styled("4 Temperature & SMART", theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 { return; }

    // Filter out virtual devices (no SMART)
    let real_devs: Vec<&BlockDevice> = devices
        .iter()
        .filter(|d| d.dev_type != DeviceType::Virtual)
        .collect();

    if real_devs.is_empty() {
        let no_devs = Paragraph::new("  No physical devices detected")
            .style(theme.text_dim);
        f.render_widget(no_devs, inner);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for dev in &real_devs {
        let temp_str = match dev.temperature() {
            Some(t) => format!("{:>3}°C", t),
            None    => "  N/A".to_string(),
        };

        let temp_style = match dev.temperature() {
            Some(t) if (dev.rotational && t >= 60) || (!dev.rotational && t >= 70) => theme.crit,
            Some(t) if (dev.rotational && t >= 50) || (!dev.rotational && t >= 55) => theme.warn,
            Some(_) => theme.ok,
            None    => theme.text_dim,
        };

        // Temperature bar (10 chars, scaled 0–80°C)
        let temp_bar = match dev.temperature() {
            Some(t) => {
                let filled = ((t.max(0) as f64 / 80.0) * 10.0).round() as usize;
                let filled = filled.min(10);
                format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled))
            }
            None => "░░░░░░░░░░".to_string(),
        };

        let (status_label, status_style) = match dev.smart_status() {
            SmartStatus::Unknown => ("    ?", theme.text_dim),
            SmartStatus::Passed  => (" PASS", theme.ok),
            SmartStatus::Warning => (" WARN", theme.warn),
            SmartStatus::Failed  => (" FAIL", theme.crit),
        };

        let poh_str = dev.smart
            .as_ref()
            .and_then(|s| s.power_on_hours)
            .map(|h| format!("  {}h", h))
            .unwrap_or_default();

        // 8-char ASCII temperature sparkline from temp_history
        const SPARKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let temp_spark: String = {
            let samples = dev.temp_history.last_n(8);
            if samples.is_empty() {
                "        ".to_string()
            } else {
                let max = samples.iter().copied().max().unwrap_or(1).max(1);
                samples.iter().map(|&v| {
                    let idx = ((v * 7) / max).min(7) as usize;
                    SPARKS[idx]
                }).collect()
            }
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<8}", dev.name), theme.text),
            Span::styled(temp_str, temp_style),
            Span::styled("  ", theme.text),
            Span::styled(temp_bar, temp_style),
            Span::styled("  ", theme.text_dim),
            Span::styled(temp_spark, temp_style),
            Span::styled(status_label, status_style),
            Span::styled(poh_str, theme.text_dim),
        ]));
    }

    // Summary footer line
    let (prefail_risk, realloc_total) = devices.iter().fold((0u32, 0u64), |(pr, rt), d| {
        let pr2 = pr + d.smart.as_ref().map(|s| {
            s.attributes.iter().filter(|a| a.is_at_risk()).count() as u32
        }).unwrap_or(0);
        let rt2 = rt + d.smart.as_ref().map(|s| {
            s.attributes.iter().find(|a| a.id == 5).map(|a| a.raw_value).unwrap_or(0)
        }).unwrap_or(0);
        (pr2, rt2)
    });

    if inner.height as usize > lines.len() + 1 {
        lines.push(Line::from(vec![]));
        lines.push(Line::from(vec![
            Span::styled("  Pre-fail at risk: ", theme.text_dim),
            Span::styled(
                format!("{}", prefail_risk),
                if prefail_risk > 0 { theme.warn } else { theme.ok },
            ),
            Span::styled("   Reallocated sectors: ", theme.text_dim),
            Span::styled(
                format!("{}", realloc_total),
                if realloc_total > 0 { theme.warn } else { theme.ok },
            ),
        ]));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}
