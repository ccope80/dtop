use crate::app::BenchmarkState;
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, state: &BenchmarkState, theme: &Theme) {
    let area = centered_rect(52, 7, f.area());
    f.render_widget(Clear, area);

    let (title, lines) = match state {
        BenchmarkState::Idle => return,

        BenchmarkState::Running(name) => (
            format!(" Benchmark — /dev/{} ", name),
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Reading 256 MiB with O_DIRECT…  ", theme.text_dim),
                ]),
                Line::from(vec![
                    Span::styled("  Press any key to cancel          ", theme.text_dim),
                ]),
            ],
        ),

        BenchmarkState::Done(name, mbs) => (
            format!(" Benchmark — /dev/{} ", name),
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Sequential Read:  ", theme.text_dim),
                    Span::styled(format!("{:.1} MB/s", mbs), theme.ok),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Press b or Esc to dismiss        ", theme.text_dim),
                ]),
            ],
        ),

        BenchmarkState::Error(name, msg) => (
            format!(" Benchmark — /dev/{} ", name),
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(format!("  Error: {}", msg), theme.crit),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Press b or Esc to dismiss        ", theme.text_dim),
                ]),
            ],
        ),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(Span::styled(title, theme.title));

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    Rect::new(
        r.x + (r.width.saturating_sub(w)) / 2,
        r.y + (r.height.saturating_sub(h)) / 2,
        w, h,
    )
}
