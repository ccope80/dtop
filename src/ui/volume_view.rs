use crate::app::App;
use crate::util::human::fmt_bytes;
use chrono::Local;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let area  = f.area();
    let theme = &app.theme;

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Header
    let now = Local::now().format("%H:%M:%S").to_string();
    let title = format!(" DTop — RAID / LVM / ZFS   {}", now);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(title, theme.title))).style(theme.header),
        root[0],
    );

    // Body: stacked sections
    let has_raid = !app.raid_arrays.is_empty();
    let has_lvm  = app.lvm_state.is_some();
    let has_zfs  = !app.zfs_pools.is_empty();

    let _sections_count = if has_raid { 1 } else { 0 }
                        + if has_lvm  { 1 } else { 0 }
                        + if has_zfs  { 1 } else { 0 }
                        + 1;  // always show "nothing detected" if all empty

    let body = root[1];

    if !has_raid && !has_lvm && !has_zfs {
        let msg = Paragraph::new(vec![
            Line::from(vec![]),
            Line::from(vec![Span::styled("  No software RAID, LVM, or ZFS detected on this system.", theme.text_dim)]),
            Line::from(vec![]),
            Line::from(vec![Span::styled("  For RAID:  check /proc/mdstat is populated (modprobe md_mod)", theme.text_dim)]),
            Line::from(vec![Span::styled("  For LVM:   install lvm2 (apt/yum install lvm2)", theme.text_dim)]),
            Line::from(vec![Span::styled("  For ZFS:   install zfsutils-linux and create a pool", theme.text_dim)]),
        ])
        .block(Block::default().borders(Borders::ALL).border_style(theme.border)
            .title(Span::styled("Volume Manager", theme.title)));
        f.render_widget(msg, body);
    } else {
        let mut constraints = Vec::new();
        if has_raid { constraints.push(Constraint::Min(5)); }
        if has_lvm  { constraints.push(Constraint::Min(6)); }
        if has_zfs  { constraints.push(Constraint::Min(5)); }

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(body);

        let mut idx = 0;

        if has_raid {
            render_raid(f, sections[idx], app);
            idx += 1;
        }
        if has_lvm {
            render_lvm(f, sections[idx], app);
            idx += 1;
        }
        if has_zfs {
            render_zfs(f, sections[idx], app);
        }
    }

    // Footer
    let footer_spans = vec![
        Span::styled(" ", theme.footer_bg),
        Span::styled(" Esc ", theme.footer_key), Span::styled("Dashboard  ", theme.footer_text),
        Span::styled(" ↑↓ ", theme.footer_key),  Span::styled("Scroll  ", theme.footer_text),
        Span::styled(" q ",  theme.footer_key),  Span::styled("Quit  ", theme.footer_text),
    ];
    f.render_widget(
        Paragraph::new(Line::from(footer_spans)).style(app.theme.footer_bg),
        root[2],
    );
}

fn render_raid(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.raid_arrays.iter().any(|a| a.degraded) { theme.warn } else { theme.border })
        .title(Span::styled("Software RAID (md)", theme.title));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for arr in &app.raid_arrays {
        let status_style = if arr.degraded { theme.crit } else { theme.ok };
        let status_dot   = if arr.degraded { "●" } else { "●" };
        let members_str  = arr.members.join(" ");

        // Usage bar (capacity display, not utilisation — just a full bar for display)
        let _bar = "████████████████".to_string();

        let rebuild_str = arr.rebuild_pct
            .map(|p| format!("  rebuilding {:.1}%", p))
            .unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<6}", arr.name), theme.text),
            Span::styled(format!("{:<8}", arr.state), theme.text_dim),
            Span::styled(format!("{:<7}", arr.level), theme.text_dim),
            Span::styled(status_dot.to_string(), status_style),
            Span::styled(format!(" {:>8}  ", fmt_bytes(arr.capacity_bytes)), theme.text_dim),
            Span::styled(format!("{:<12}", arr.bitmap), status_style),
            Span::styled(format!("  {}", members_str), theme.text_dim),
            Span::styled(rebuild_str, theme.warn),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_lvm(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("LVM Volume Groups", theme.title));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lvm = match &app.lvm_state {
        Some(l) => l,
        None    => return,
    };

    let mut lines: Vec<Line> = Vec::new();
    for vg in &lvm.vgs {
        let pct  = vg.use_pct();
        let style = theme.util_style(pct);

        let filled = ((pct / 100.0) * 16.0).round() as usize;
        let filled = filled.min(16);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(16 - filled));

        lines.push(Line::from(vec![
            Span::styled(format!("  VG {:<12}", vg.name), theme.title),
            Span::styled(format!("{} PVs  {} LVs  ", vg.pv_count, vg.lv_count), theme.text_dim),
            Span::styled(bar, style),
            Span::styled(format!(" {:>5.1}%", pct), style),
            Span::styled(format!("  {}  free: {}", fmt_bytes(vg.size_bytes), fmt_bytes(vg.free_bytes)), theme.text_dim),
        ]));

        // PVs belonging to this VG
        for pv in lvm.pvs.iter().filter(|p| p.vg_name == vg.name) {
            lines.push(Line::from(vec![
                Span::styled(format!("    PV {}", pv.name), theme.text_dim),
                Span::styled(format!("  {}", fmt_bytes(pv.size_bytes)), theme.text_dim),
            ]));
        }

        // LVs belonging to this VG
        for lv in lvm.lvs.iter().filter(|l| l.vg_name == vg.name) {
            // Try to find mount/filesystem info
            let mount_info = app.filesystems.iter()
                .find(|fs| fs.device == lv.path || fs.device.ends_with(&format!("/{}", lv.name)))
                .map(|fs| format!("  {}  {:.0}%", fs.mount, fs.use_pct()))
                .unwrap_or_default();

            lines.push(Line::from(vec![
                Span::styled(format!("    LV {:<16}", lv.name), theme.text),
                Span::styled(format!("{:>10}", fmt_bytes(lv.size_bytes)), theme.text_dim),
                Span::styled(mount_info, theme.text_dim),
            ]));
        }

        lines.push(Line::from(vec![]));
    }

    f.render_widget(
        Paragraph::new(lines).scroll((app.volume_scroll as u16, 0)),
        inner,
    );
}

fn render_zfs(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("ZFS Pools", theme.title));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for pool in &app.zfs_pools {
        let pct   = pool.use_pct();
        let style = if pool.is_healthy() { theme.util_style(pct) } else { theme.crit };
        let filled = ((pct / 100.0) * 16.0).round() as usize;
        let filled = filled.min(16);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(16 - filled));

        let health_style = if pool.is_healthy() { theme.ok } else { theme.crit };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<12}", pool.name), theme.title),
            Span::styled(format!("{:<9}", pool.health), health_style),
            Span::styled(bar, style),
            Span::styled(format!(" {:>5.1}%", pct), style),
            Span::styled(
                format!("  {}  alloc: {}  free: {}",
                    fmt_bytes(pool.size_bytes),
                    fmt_bytes(pool.alloc_bytes),
                    fmt_bytes(pool.free_bytes)),
                theme.text_dim,
            ),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
