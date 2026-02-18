use crate::app::App;
use crate::util::human::{fmt_bytes, fmt_eta};
use chrono::Local;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
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
    let title = format!(" DTop — Filesystem Overview   {}", now);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(title, theme.title))).style(theme.header),
        root[0],
    );

    // Table
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled("All Mounted Filesystems", theme.title));
    let inner = block.inner(root[1]);
    f.render_widget(block, root[1]);

    let header_cells = ["Mount", "Type", "Size", "Used", "Avail", "Use%", "Inode%", "Fill/day", "ETA", "Device"]
        .iter()
        .map(|h| Cell::from(*h).style(theme.text_dim));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = app.filesystems.iter().map(|fs| {
        let pct   = fs.use_pct();
        let ipct  = fs.inode_pct();
        let style = theme.util_style(pct);

        let inode_str = if fs.total_inodes == 0 {
            " -".to_string()
        } else {
            format!("{:.0}%", ipct)
        };

        let alert = if pct >= 95.0 || ipct >= 95.0 { " !!" }
                    else if pct >= 85.0 || ipct >= 85.0 { " !" }
                    else { "" };

        // Fill rate: "+1.2 GB" per day, or "—"
        let (rate_str, rate_style) = match fs.fill_rate_bps {
            Some(r) if r > 1024.0 => {
                let day = r * 86_400.0;
                let eta_style = match fs.days_until_full {
                    Some(d) if d < 3.0  => theme.crit,
                    Some(d) if d < 14.0 => theme.warn,
                    _                   => theme.text_dim,
                };
                (format!("+{}", fmt_bytes(day as u64)), eta_style)
            }
            Some(r) if r < -1024.0 => {
                let day = (-r) * 86_400.0;
                (format!("-{}", fmt_bytes(day as u64)), theme.ok)
            }
            _ => ("—".to_string(), theme.text_dim),
        };

        let eta_str = match fs.days_until_full {
            Some(d) if fs.fill_rate_bps.map_or(false, |r| r > 0.0) => fmt_eta(d),
            _ => "—".to_string(),
        };
        let eta_style = match fs.days_until_full {
            Some(d) if d < 3.0  => theme.crit,
            Some(d) if d < 14.0 => theme.warn,
            _                   => theme.text_dim,
        };

        Row::new(vec![
            Cell::from(fs.mount.clone()),
            Cell::from(fs.fs_type.clone()).style(theme.text_dim),
            Cell::from(fmt_bytes(fs.total_bytes)).style(theme.text_dim),
            Cell::from(fmt_bytes(fs.used_bytes)).style(style),
            Cell::from(fmt_bytes(fs.avail_bytes)).style(theme.text_dim),
            Cell::from(format!("{:.0}%{}", pct, alert)).style(style),
            Cell::from(inode_str).style(if ipct >= 85.0 { theme.warn } else { theme.text_dim }),
            Cell::from(rate_str).style(rate_style),
            Cell::from(eta_str).style(eta_style),
            Cell::from(fs.device.clone()).style(theme.text_dim),
        ])
    }).collect();

    let widths = [
        Constraint::Min(16),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Length(9),
        Constraint::Length(6),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .row_highlight_style(theme.selected);

    f.render_stateful_widget(table, inner, &mut app.fs_table_state);

    // Footer
    let footer_spans = vec![
        Span::styled(" ", theme.footer_bg),
        Span::styled(" Esc ", theme.footer_key), Span::styled("Dashboard  ", theme.footer_text),
        Span::styled(" ↑↓ ", theme.footer_key),  Span::styled("Scroll  ", theme.footer_text),
        Span::styled(" q ",  theme.footer_key),  Span::styled("Quit  ", theme.footer_text),
    ];
    f.render_widget(
        Paragraph::new(Line::from(footer_spans)).style(theme.footer_bg),
        root[2],
    );
}
