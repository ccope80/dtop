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

    // ── Root: header | body | footer ───────────────────────────────
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // ── Header ─────────────────────────────────────────────────────
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

    let now  = Local::now().format("%H:%M:%S").to_string();
    let left = format!(" DTop v0.1 — {} ", app.theme_variant.name());
    let right = format!(" {} ", now);
    let pad  = (area.width as usize)
        .saturating_sub(left.len() + alert_badge.len() + right.len());

    let header = Line::from(vec![
        Span::styled(left, theme.title),
        Span::styled(alert_badge, alert_style),
        Span::styled(" ".repeat(pad), theme.header),
        Span::styled(right, theme.text_dim),
    ])
    .style(theme.header);
    f.render_widget(Paragraph::new(header), root[0]);

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
            app.active_panel == ActivePanel::Devices, &theme,
        );

        if let Some(idx) = app.device_list_state.selected() {
            if let Some(dev) = app.devices.get(idx) {
                render_detail(f, cols[1], dev, app.detail_scroll, app.detail_history_window, &theme);
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
        app.active_panel == ActivePanel::Devices, theme,
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
        app.active_panel == ActivePanel::Devices, theme,
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
        app.active_panel == ActivePanel::Devices, theme,
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
        app.active_panel == ActivePanel::Devices, theme,
    );
    render_filesystem_bars(
        f, rows[1], &app.filesystems, app.fs_scroll,
        app.active_panel == ActivePanel::Filesystem, theme,
    );
}
