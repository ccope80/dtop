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
    let crit_count = app.alerts.iter()
        .filter(|a| a.severity == crate::alerts::Severity::Critical)
        .count();
    let warn_count = app.alerts.iter()
        .filter(|a| a.severity == crate::alerts::Severity::Warning)
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
    let pad   = (area.width as usize)
        .saturating_sub(left.len() + alert_badge.len() + right.len());

    let header_line1 = Line::from(vec![
        Span::styled(left, theme.title),
        Span::styled(alert_badge, alert_style),
        Span::styled(" ".repeat(pad), theme.header),
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
    fleet_spans.push(Span::styled(avg_suffix, avg_style));

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
            app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), &theme,
        );

        if let Some(idx) = app.device_list_state.selected() {
            if let Some(dev) = app.devices.get(idx) {
                let test_status = app.smart_test_status.get(&dev.name).map(|s| s.as_str());
                let anomalies   = app.smart_anomalies.get(&dev.name);
                render_detail(f, cols[1], dev, app.detail_scroll, app.detail_history_window, test_status, anomalies, &theme);
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
    render_footer(f, root[2], &app.active_panel, app.layout_preset, &theme);
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

    app.device_list_area = Some(top[0]);
    render_device_list(
        f, top[0], &app.devices, &mut app.device_list_state,
        app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), theme,
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
        f, bottom[1], &app.alerts, &app.alert_history,
        app.active_panel == ActivePanel::Alerts, theme,
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

    app.device_list_area = Some(top[0]);
    render_device_list(
        f, top[0], &app.devices, &mut app.device_list_state,
        app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), theme,
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

// ── Preset 2: Storage — devices left, full filesystem right ──────────────

fn render_preset_storage(f: &mut Frame, body: ratatui::layout::Rect, app: &mut App, theme: &crate::ui::theme::Theme) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(body);

    app.device_list_area = Some(cols[0]);
    render_device_list(
        f, cols[0], &app.devices, &mut app.device_list_state,
        app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), theme,
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

    app.device_list_area = Some(rows[0]);
    render_device_list(
        f, rows[0], &app.devices, &mut app.device_list_state,
        app.active_panel == ActivePanel::Devices, app.device_filter.label(), app.device_sort.label(), theme,
    );
    render_filesystem_bars(
        f, rows[1], &app.filesystems, app.fs_scroll,
        app.active_panel == ActivePanel::Filesystem, theme,
    );
}
