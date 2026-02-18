use crate::ui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, theme: &Theme, scroll: usize) {
    let area = centered_rect(70, 34, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled(" DTop — Keybindings (? or F1 to close) ", theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into two columns
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left = vec![
        key_line(theme, "Global", ""),
        key_line(theme, "  q / Ctrl-C",     "Quit"),
        key_line(theme, "  Esc / h",        "Back / Dashboard"),
        key_line(theme, "  Tab / Shift-Tab","Focus next / prev panel"),
        key_line(theme, "  ↑↓ / j k",      "Select / scroll"),
        key_line(theme, "  g / G",          "Jump first / last"),
        key_line(theme, "  Enter / l",      "Drill-down / confirm"),
        key_line(theme, "  PageUp/Dn",      "Scroll list"),
        key_line(theme, "  t",              "Cycle color theme"),
        key_line(theme, "  C",              "Config viewer overlay"),
        key_line(theme, "  ? / F1",         "Toggle this help"),
        Line::from(""),
        key_line(theme, "Views", ""),
        key_line(theme, "  F2",  "Process I/O view"),
        key_line(theme, "  F3",  "Filesystem overview (fill rate + ETA)"),
        key_line(theme, "  F4",  "RAID / LVM / ZFS view"),
        key_line(theme, "  F5",  "NFS mount latency view"),
        key_line(theme, "  F6",  "Alert log viewer (full history, s=filter)"),
        Line::from(""),
        key_line(theme, "Dashboard — Device list", ""),
        key_line(theme, "  Enter / click×2", "Open / close device detail"),
        key_line(theme, "  f",     "Cycle filter (All / NVMe / SSD / HDD)"),
        key_line(theme, "  s",     "Cycle sort (Natural / Util / Temp / Health)"),
        key_line(theme, "  p",     "Cycle layout (Full / IO-Focus / Storage)"),
        key_line(theme, "  a",     "Acknowledge all active alerts"),
        Line::from(""),
        key_line(theme, "Device detail pane", ""),
        key_line(theme, "  w",  "Cycle history window (60s / 5m / 1h)"),
        key_line(theme, "  r",  "Force SMART re-poll now"),
        key_line(theme, "  B",  "Save SMART baseline snapshot"),
        key_line(theme, "  D",  "Toggle SMART attribute descriptions"),
        key_line(theme, "  b",  "Sequential read benchmark (256 MiB)"),
        key_line(theme, "  x",  "Schedule SMART short self-test"),
    ];

    let right = vec![
        key_line(theme, "Mouse", ""),
        key_line(theme, "  Click",         "Select device"),
        key_line(theme, "  Click (sel'd)", "Toggle detail open / close"),
        key_line(theme, "  Scroll",        "Scroll active panel"),
        Line::from(""),
        key_line(theme, "Process I/O (F2)", ""),
        key_line(theme, "  s",    "Cycle sort column"),
        key_line(theme, "  ↑↓",  "Navigate"),
        Line::from(""),
        key_line(theme, "Filesystem (F3)", ""),
        key_line(theme, "  ↑↓",  "Scroll table (shows fill rate + ETA)"),
        Line::from(""),
        key_line(theme, "Volume Manager (F4)", ""),
        key_line(theme, "  ↑↓",  "Scroll list"),
        Line::from(""),
        key_line(theme, "CLI modes", ""),
        key_line(theme, "  --check",       "Exit 0=OK 1=WARN 2=CRIT (nagios)"),
        key_line(theme, "  --summary",     "One-line status (exit 0/1/2)"),
        key_line(theme, "  --watch N",     "Rolling status every N seconds"),
        key_line(theme, "  --report",      "Human-readable health report"),
        key_line(theme, "  --report-html", "Self-contained HTML report"),
        key_line(theme, "  --json",        "JSON snapshot and exit"),
        key_line(theme, "  --csv",         "Device snapshot as CSV"),
        key_line(theme, "  --diff A B",    "Compare two --json snapshots"),
        key_line(theme, "  --daemon",      "Headless alert daemon"),
        key_line(theme, "  --alerts",            "Show recent alert log entries"),
        key_line(theme, "  --alerts --since Nd", "Filter alerts by age (24h, 7d…)"),
        key_line(theme, "  --top-io",            "Top processes by disk I/O"),
        key_line(theme, "  --device-report DEV", "Full SMART report for one device"),
        key_line(theme, "  --anomalies",         "Show tracked SMART anomaly log"),
        key_line(theme, "  --endurance",         "Write endurance per device"),
        key_line(theme, "  --baselines",         "List saved SMART baselines"),
        key_line(theme, "  --schedule-test DEV", "Schedule SMART self-test"),
        key_line(theme, "  --save-baseline DEV", "Save SMART baseline (no TUI)"),
        key_line(theme, "  --clear-anomalies",   "Clear anomaly log [--yes]"),
        key_line(theme, "  --io-sched [DEV[=S]]","View/set I/O scheduler"),
        key_line(theme, "  --top-temp",          "Devices by temperature (cache)"),
        key_line(theme, "  --spindown DEV",      "HDD standby via hdparm [-y/-Y]"),
        key_line(theme, "  --trim [MOUNT]",      "Run fstrim on fs (or all)"),
        key_line(theme, "  --apm DEV[=LEVEL]",   "View/set HDD APM (1-255)"),
        key_line(theme, "  --report-md",         "Markdown health report"),
        key_line(theme, "  --bench DEV[--size N]","Sequential read benchmark (CLI)"),
        key_line(theme, "  --health-history DEV","Health score trend [--days N]"),
        key_line(theme, "  --forecast",          "Filesystem fill-rate + ETA table"),
        key_line(theme, "  --iostat [DEV]",     "Rolling device I/O stats (--count N)"),
        key_line(theme, "  --capacity",         "Device capacity inventory table"),
        key_line(theme, "  --smart-attr D ATTR","Lookup one SMART attribute (ID/name)"),
        key_line(theme, "  --disk-info DEV",   "Sysfs device parameters panel"),
        key_line(theme, "  --power-state [DEV]","HDD power state via hdparm -C"),
        key_line(theme, "  --cumulative-io [D]","Total I/O since boot (ops + latency)"),
        key_line(theme, "  --lsof DEV|MOUNT",  "Processes with open files on target"),
        key_line(theme, "  --blkid",           "UUIDs, labels, FS types (blkid)"),
        key_line(theme, "  --mount",           "Active mounts with key options"),
        key_line(theme, "  --dmesg [DEV]",    "Kernel storage msgs (--last N)"),
        key_line(theme, "  --verify DEV",     "Read-verify pass (--size N MiB)"),
        key_line(theme, "  --partition-table","Partition layout + UUID/FS/mount"),
        key_line(theme, "  --print-service",     "Print systemd unit for daemon"),
        key_line(theme, "  --test-webhook",      "Send test webhook notification"),
        key_line(theme, "  --edit-config",       "Open config in $EDITOR"),
        key_line(theme, "  --config",            "Print current config values"),
        key_line(theme, "  --no-smart",          "Disable SMART polling"),
        key_line(theme, "  --completions",       "Shell completion script"),
        Line::from(""),
        key_line(theme, "Config  ~/.config/dtop/dtop.toml", ""),
        key_line(theme, "  Hot-reloaded on change (30 s poll)", ""),
        key_line(theme, "  Acks/logs  ~/.local/share/dtop/", ""),
    ];

    let s = scroll as u16;
    f.render_widget(Paragraph::new(left).scroll((s, 0)), cols[0]);
    f.render_widget(Paragraph::new(right).scroll((s, 0)), cols[1]);
}

fn key_line<'a>(theme: &Theme, key: &'a str, desc: &'a str) -> Line<'a> {
    if desc.is_empty() {
        // Section header
        Line::from(vec![
            Span::styled(key, theme.title),
        ])
    } else {
        Line::from(vec![
            Span::styled(format!("{:<22}", key), theme.footer_key),
            Span::styled(desc, theme.text_dim),
        ])
    }
}

/// Returns a centered Rect of `pct_w`% width and `pct_h`% height,
/// but capped at the available area.
fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Helper that splits and discards to achieve centering by percentage (unused but kept for reference).
#[allow(dead_code)]
fn centered_pct(pct_w: u16, pct_h: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_h) / 2),
            Constraint::Percentage(pct_h),
            Constraint::Percentage((100 - pct_h) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_w) / 2),
            Constraint::Percentage(pct_w),
            Constraint::Percentage((100 - pct_w) / 2),
        ])
        .split(popup_layout[1])[1]
}
