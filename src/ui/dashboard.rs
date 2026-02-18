use crate::app::{ActivePanel, App};
use crate::ui::{
    alerts_panel::render_alerts_panel,
    detail::render_detail,
    device_list::render_device_list,
    filesystem_bars::render_filesystem_bars,
    footer::render_footer,
    smart_panel::render_smart_panel,
    throughput::render_throughput,
};
use crate::util::health_score::{health_score, score_style};
use crate::util::human::fmt_rate;
use chrono::Local;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

// Layout presets (Dashboard only)
// 0 = Full:    5-panel (devices+throughput | filesystem | smart+alerts)
// 1 = IO-Focus: devices+throughput top (larger) | filesystem bottom
// 2 = Storage:  devices (left 35%) | filesystem (right 65%), no throughput/smart/alerts

pub fn render(f: &mut Frame, app: &mut App) {
    let area  = f.area();
    let theme = app.theme.clone();

    // ── Root: header (2 lines) | body | footer ─────────────────────
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // ── Header line 1: title + alerts + clock ──────────────────────
    // Count only un-acked alerts for the badge
    let crit_count = app.alerts.iter()
        .filter(|a| a.severity == crate::alerts::Severity::Critical && !app.acked_alerts.contains(&a.key()))
        .count();
    let warn_count = app.alerts.iter()
        .filter(|a| a.severity == crate::alerts::Severity::Warning && !app.acked_alerts.contains(&a.key()))
        .count();

    let alert_badge = if crit_count > 0 {
        format!("  !! {} CRIT  {} WARN  ", crit_count, warn_count)
    } else if warn_count > 0 {
        format!("  {} WARN  ", warn_count)
    } else {
        "  ".to_string()
    };

    let alert_style = if crit_count > 0 { theme.crit }
                      else if warn_count > 0 { theme.warn }
                      else { theme.text_dim };

    let now   = Local::now().format("%H:%M:%S").to_string();
    let left  = format!(" DTop v0.1 — {} ", app.theme_variant.name());
    let right = format!(" {} ", now);

    let reload_flash = match app.config_reload_flash {
        Some(t) if t.elapsed() < std::time::Duration::from_secs(3) => "  ↺ Config reloaded  ",
        _ => "",
    };

    let pad = (area.width as usize)
        .saturating_sub(left.len() + alert_badge.len() + reload_flash.len() + right.len());

    let header_line1 = Line::from(vec![
        Span::styled(left, theme.title),
        Span::styled(alert_badge, alert_style),
        Span::styled(" ".repeat(pad), theme.header),
        Span::styled(reload_flash, theme.ok),
        Span::styled(right, theme.text_dim),
    ]);

    // ── Header line 2: fleet I/O + health distribution ─────────────
    let total_read:  f64 = app.devices.iter().map(|d| d.read_bytes_per_sec).sum();
    let total_write: f64 = app.devices.iter().map(|d| d.write_bytes_per_sec).sum();
    let n = app.devices.len();
    let mut ok_n = 0usize; let mut warn_n = 0usize; let mut crit_n = 0usize;
    let mut score_sum = 0u32;
    for d in &app.devices {
        let s = health_score(d);
        score_sum += s as u32;
        if s >= 80 { ok_n += 1; } else if s >= 50 { warn_n += 1; } else { crit_n += 1; }
    }
    let avg_score = if n > 0 { (score_sum / n as u32) as u8 } else { 100 };
    let avg_style = score_style(avg_score, &theme);

    let fleet_prefix = format!(" Fleet  R:{:>9}  W:{:>9}   {} devs  ", fmt_rate(total_read), fmt_rate(total_write), n);
    let avg_suffix   = format!("  avg health: {}/100 ", avg_score);

    let mut fleet_spans = vec![Span::styled(fleet_prefix, theme.text_dim)];
    if ok_n > 0   { fleet_spans.push(Span::styled(format!("{}●", ok_n),   theme.ok));  fleet_spans.push(Span::styled(" ", theme.text_dim)); }
    if warn_n > 0 { fleet_spans.push(Span::styled(format!("{}●", warn_n), theme.warn)); fleet_spans.push(Span::styled(" ", theme.text_dim)); }
    if crit_n > 0 { fleet_spans.push(Span::styled(format!("{}●", crit_n), theme.crit)); fleet_spans.push(Span::styled(" ", theme.text_dim)); }
    // Fleet health bar: 10 chars, proportional fill of ok(green)/warn(yellow)/crit(red)
    if n > 0 {
        let bar_w = 10usize;
        let ok_fill   = ((ok_n   as f64 / n as f64) * bar_w as f64).round() as usize;
        let warn_fill = ((warn_n as f64 / n as f64) * bar_w as f64).round() as usize;
        let crit_fill = bar_w.saturating_sub(ok_fill + warn_fill);
        fleet_spans.push(Span::styled(" [".to_string(), theme.text_dim));
        if ok_fill   > 0 { fleet_spans.push(Span::styled("█".repeat(ok_fill),   theme.ok)); }
        if warn_fill > 0 { fleet_spans.push(Span::styled("█".repeat(warn_fill), theme.warn)); }
        if crit_fill > 0 { fleet_spans.push(Span::styled("█".repeat(crit_fill), theme.crit)); }
        fleet_spans.push(Span::styled("] ".to_string(), theme.text_dim));
    }
    fleet_spans.push(Span::styled(avg_suffix, avg_style));

    // PSI I/O pressure (appended if available)
    if let Some(psi) = &app.system_pressure {
        let some = psi.io.some.avg10;
        let full = psi.io.full.avg10;
        let psi_style = if some >= 50.0 || full >= 20.0 { theme.crit }
                        else if some >= 20.0 || full >= 5.0  { theme.warn }
                        else { theme.text_dim };
        fleet_spans.push(Span::styled(
            format!("   io psi: {:.1}% ", some),
            psi_style,
        ));
        if full > 0.1 {
            fleet_spans.push(Span::styled(
                format!("full:{:.1}% ", full),
                theme.crit,
            ));
        }
    }

    let header_line2 = Line::from(fleet_spans);

    f.render_widget(
        Paragraph::new(vec![header_line1, header_line2]).style(theme.header),
        root[0],
    );

    let body = root[1];

    // ── Body: detail view or dashboard ─────────────────────────────
    if app.detail_open {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(36), Constraint::Min(0)])
            .split(body);

        app.device_list_area = Some(cols[0]);
        render_device_list(
            f, cols[0], &app.devices, &mut app.device_list_state,
            app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), &app.health_history, &app.device_io_history, crit_count, warn_count, &theme,
        );

        if let Some(idx) = app.device_list_state.selected() {
            if let Some(dev) = app.devices.get(idx) {
                let test_status = app.smart_test_status.get(&dev.name).map(|s| s.as_str());
                let anomalies   = app.smart_anomalies.get(&dev.name);
                let baseline    = app.smart_baselines.get(&dev.name).map(|b| b as &_);
                let endurance   = app.write_endurance.get(&dev.name).map(|e| e as &_);
                render_detail(f, cols[1], dev, &app.filesystems, app.detail_scroll, app.detail_history_window, test_status, anomalies, baseline, endurance, app.detail_show_desc, &theme);
            }
        }
    } else if area.width < 100 {
        // ── Compact mode (narrow terminal) ────────────────────────
        render_compact(f, body, app, &theme);
    } else {
        // ── Normal dashboard — layout determined by preset ────────
        match app.layout_preset {
            1 => render_preset_io_focus(f, body, app, &theme),
            2 => render_preset_storage(f, body, app, &theme),
            _ => render_preset_full(f, body, app, &theme),
        }
    }

    // ── Footer ─────────────────────────────────────────────────────
    render_footer(f, root[2], &app.active_panel, app.layout_preset, &theme, &app.active_view, app.detail_open);
}

// ── Preset 0: Full 5-panel layout (default) ────────────────────────────

fn render_preset_full(f: &mut Frame, body: ratatui::layout::Rect, app: &mut App, theme: &crate::ui::theme::Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(44),
            Constraint::Percentage(28),
            Constraint::Percentage(28),
        ])
        .split(body);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(rows[0]);

    let (nc, nw) = alert_badge_counts(app);
    app.device_list_area = Some(top[0]);
    render_device_list(
        f, top[0], &app.devices, &mut app.device_list_state,
        app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), &app.health_history, &app.device_io_history, nc, nw, theme,
    );
    render_throughput(
        f, top[1], &app.devices,
        app.active_panel == ActivePanel::Throughput, theme,
    );

    render_filesystem_bars(
        f, rows[1], &app.filesystems, app.fs_scroll,
        app.active_panel == ActivePanel::Filesystem, theme,
    );

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);

    render_smart_panel(
        f, bottom[0], &app.devices,
        app.active_panel == ActivePanel::SmartTemp, theme,
    );
    render_alerts_panel(
        f, bottom[1], &app.alerts, &app.alert_history, &app.acked_alerts,
        app.active_panel == ActivePanel::Alerts, theme,
        &mut app.alerts_panel_state,
    );
}

// ── Preset 1: IO-Focus — large top (devices+throughput), filesystem below ──

fn render_preset_io_focus(f: &mut Frame, body: ratatui::layout::Rect, app: &mut App, theme: &crate::ui::theme::Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(body);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[0]);

    let (nc, nw) = alert_badge_counts(app);
    app.device_list_area = Some(top[0]);
    render_device_list(
        f, top[0], &app.devices, &mut app.device_list_state,
        app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), &app.health_history, &app.device_io_history, nc, nw, theme,
    );
    render_throughput(
        f, top[1], &app.devices,
        app.active_panel == ActivePanel::Throughput, theme,
    );

    render_filesystem_bars(
        f, rows[1], &app.filesystems, app.fs_scroll,
        app.active_panel == ActivePanel::Filesystem, theme,
    );
}

fn alert_badge_counts(app: &App) -> (usize, usize) {
    let nc = app.alerts.iter()
        .filter(|a| a.severity == crate::alerts::Severity::Critical && !app.acked_alerts.contains(&a.key()))
        .count();
    let nw = app.alerts.iter()
        .filter(|a| a.severity == crate::alerts::Severity::Warning && !app.acked_alerts.contains(&a.key()))
        .count();
    (nc, nw)
}

// ── Preset 2: Storage — devices left, full filesystem right ──────────────

fn render_preset_storage(f: &mut Frame, body: ratatui::layout::Rect, app: &mut App, theme: &crate::ui::theme::Theme) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(body);

    let (nc, nw) = alert_badge_counts(app);
    app.device_list_area = Some(cols[0]);
    render_device_list(
        f, cols[0], &app.devices, &mut app.device_list_state,
        app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), &app.health_history, &app.device_io_history, nc, nw, theme,
    );
    render_filesystem_bars(
        f, cols[1], &app.filesystems, app.fs_scroll,
        app.active_panel == ActivePanel::Filesystem, theme,
    );
}

// ── Compact mode (width < 100): stacked single column ──────────────────────

fn render_compact(f: &mut Frame, body: ratatui::layout::Rect, app: &mut App, theme: &crate::ui::theme::Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(body);

    let (nc, nw) = alert_badge_counts(app);
    app.device_list_area = Some(rows[0]);
    render_device_list(
        f, rows[0], &app.devices, &mut app.device_list_state,
        app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), &app.health_history, &app.device_io_history, nc, nw, theme,
    );
    render_filesystem_bars(
        f, rows[1], &app.filesystems, app.fs_scroll,
        app.active_panel == ActivePanel::Filesystem, theme,
    );
}
