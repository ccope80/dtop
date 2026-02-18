use crate::models::filesystem::Filesystem;
use crate::ui::theme::Theme;
use crate::util::human::{fmt_bytes, fmt_eta};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

pub fn render_filesystem_bars(
    f: &mut Frame,
    area: Rect,
    filesystems: &[Filesystem],
    scroll: usize,
    focused: bool,
    theme: &Theme,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled("3 Filesystem Usage", theme.title));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || filesystems.is_empty() { return; }

    // Each filesystem takes 2 rows: label row + gauge row
    let rows_per_fs = 2usize;
    let visible = (inner.height as usize / rows_per_fs).max(1);
    let start = scroll.min(filesystems.len().saturating_sub(1));
    let end   = (start + visible).min(filesystems.len());
    let visible_fs = &filesystems[start..end];

    let constraints: Vec<Constraint> = visible_fs
        .iter()
        .flat_map(|_| [Constraint::Length(1), Constraint::Length(1)])
        .collect();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, fs) in visible_fs.iter().enumerate() {
        let label_row = rows[i * 2];
        let gauge_row = rows[i * 2 + 1];

        let pct = fs.use_pct();
        let inode_pct = fs.inode_pct();
        let gauge_style = theme.util_style(pct);

        // Label: mount  type  used/total  inode%
        let inode_str = if inode_pct > 50.0 {
            format!("  inodes:{:.0}%", inode_pct)
        } else {
            String::new()
        };

        let alert = if pct >= 95.0 { " !!" } else if pct >= 85.0 { " !" } else { "" };

        // Fill rate hint: "+1.2 MB/day · full ~45d" or "" if stable/shrinking
        let fill_hint = match (fs.fill_rate_bps, fs.days_until_full) {
            (Some(rate), Some(eta)) if rate > 1024.0 => {
                let rate_day = rate * 86_400.0;
                format!("  +{}/day  full ~{}", fmt_bytes(rate_day as u64), fmt_eta(eta))
            }
            _ => String::new(),
        };
        let fill_style = match fs.days_until_full {
            Some(d) if d < 3.0  => theme.crit,
            Some(d) if d < 14.0 => theme.warn,
            _                   => theme.text_dim,
        };

        let label = Line::from(vec![
            Span::styled(format!("{:<20}", fs.mount), theme.text),
            Span::styled(format!("{:<6}", fs.fs_type), theme.text_dim),
            Span::styled(format!(" {:>8}", fmt_bytes(fs.total_bytes)), theme.text_dim),
            Span::styled(
                format!("  used {}", fmt_bytes(fs.used_bytes)),
                gauge_style,
            ),
            Span::styled(format!("  avail {}", fmt_bytes(fs.avail_bytes)), theme.text_dim),
            Span::styled(inode_str, theme.warn),
            Span::styled(alert, theme.crit),
            Span::styled(fill_hint, fill_style),
        ]);
        f.render_widget(Paragraph::new(label), label_row);

        // Gauge bar
        let gauge = Gauge::default()
            .gauge_style(gauge_style)
            .ratio((pct / 100.0).clamp(0.0, 1.0))
            .label(format!("{:.0}%", pct));
        f.render_widget(gauge, gauge_row);
    }
}
