use crate::config::Config;
use crate::ui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_config_overlay(f: &mut Frame, config: &Config, theme: &Theme) {
    let area = centered_rect(76, 38, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled(" DTop — Current Config  (C to close) ", theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let path_str = Config::config_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown)".to_string());

    let t = &config.alerts.thresholds;

    let left_strs = [
        ("Config file",       path_str.clone(),   false),
        ("",                  String::new(),       false),
        ("General",           String::new(),       true),
        ("Update interval",   format!("{} ms", config.general.update_interval_ms), false),
        ("SMART interval",    format!("{} s",  config.general.smart_interval_sec),  false),
        ("",                  String::new(),       false),
        ("Alert thresholds",  String::new(),       true),
        ("FS warn / crit",    format!("{:.0}% / {:.0}%", t.filesystem_warn_pct, t.filesystem_crit_pct), false),
        ("Inode warn / crit", format!("{:.0}% / {:.0}%", t.inode_warn_pct, t.inode_crit_pct), false),
        ("Temp SSD warn/crit",format!("{}°C / {}°C", t.temperature_warn_ssd, t.temperature_crit_ssd), false),
        ("Temp HDD warn/crit",format!("{}°C / {}°C", t.temperature_warn_hdd, t.temperature_crit_hdd), false),
        ("I/O util warn",     format!("{:.0}%", t.io_util_warn_pct), false),
        ("Latency warn/crit", format!("{:.0}ms / {:.0}ms", t.latency_warn_ms, t.latency_crit_ms), false),
        ("Fill-rate warn/crit", {
            let w = if t.fill_days_warn > 0.0 { format!("{:.0}d", t.fill_days_warn) } else { "off".into() };
            let c = if t.fill_days_crit > 0.0 { format!("{:.0}d", t.fill_days_crit) } else { "off".into() };
            format!("{} / {}", w, c)
        }, false),
        ("",                  String::new(),       false),
        ("Alert cooldown",    String::new(),       true),
        ("Cooldown",          format!("{} h  (0 = no cooldown)", config.alerts.cooldown_hours), false),
    ];

    let left: Vec<Line> = left_strs.iter().map(|(key, val, is_header)| {
        if *is_header {
            hdr(key, theme)
        } else if key.is_empty() {
            Line::from("")
        } else {
            kv(key, val, theme)
        }
    }).collect();

    let webhook_display = if config.notifications.webhook_url.is_empty() {
        "(not configured)".to_string()
    } else {
        let url = &config.notifications.webhook_url;
        format!("{}…(masked)", &url[..url.len().min(16)])
    };

    let exclude_str = if config.devices.exclude.is_empty() {
        "(none)".to_string()
    } else {
        config.devices.exclude.join(", ")
    };

    let data_dir = dirs::data_local_dir()
        .map(|p| p.join("dtop").to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown)".to_string());

    let mut right: Vec<Line> = vec![
        hdr("Notifications", theme),
        kv("Webhook URL",    &webhook_display, theme),
        kv("Notify on CRIT", if config.notifications.notify_critical { "yes" } else { "no" }, theme),
        kv("Notify on WARN", if config.notifications.notify_warning  { "yes" } else { "no" }, theme),
        kv("notify-send",    if config.notifications.notify_send     { "enabled" } else { "disabled" }, theme),
        Line::from(""),
        hdr("Device exclusions", theme),
        dim(&exclude_str, theme),
        Line::from(""),
        hdr("Aliases", theme),
    ];
    if config.devices.aliases.is_empty() {
        right.push(dim("(no aliases)", theme));
    } else {
        for (k, v) in &config.devices.aliases {
            right.push(dim(&format!("  {} → {}", k, v), theme));
        }
    }
    right.push(Line::from(""));
    right.push(hdr("Data directory", theme));
    right.push(dim(&data_dir, theme));
    right.push(Line::from(""));
    right.push(hdr("SMART alert rules", theme));
    let rules_str = if config.alerts.smart_rules.is_empty() {
        "0 rules (all disabled)".to_string()
    } else {
        let w = config.alerts.smart_rules.iter().filter(|r| r.severity == "warn").count();
        let c = config.alerts.smart_rules.iter().filter(|r| r.severity != "warn").count();
        format!("{} rule(s) — {} warn / {} crit", config.alerts.smart_rules.len(), w, c)
    };
    right.push(dim(&rules_str, theme));
    for rule in &config.alerts.smart_rules {
        let msg = rule.message.as_deref().unwrap_or("(auto)");
        right.push(dim(&format!("  attr {:>3}  {} {}  [{}]  {}", rule.attr, rule.op, rule.value, rule.severity, msg), theme));
    }

    f.render_widget(Paragraph::new(left).wrap(Wrap { trim: false }), cols[0]);
    f.render_widget(Paragraph::new(right).wrap(Wrap { trim: false }), cols[1]);
}

fn hdr(title: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(title.to_string(), theme.title)])
}

fn kv(key: &str, val: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<22}", key), theme.text_dim),
        Span::styled(val.to_string(), theme.text),
    ])
}

fn dim(val: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(format!("  {}", val), theme.text_dim)])
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}
