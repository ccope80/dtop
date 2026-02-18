use ratatui::style::{Color, Modifier, Style};

// ── Helper: build an Rgb Color from a hex literal ──────────────────────

const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >>  8) & 0xFF) as u8,
        ( hex        & 0xFF) as u8,
    )
}

// ── Theme variant selector ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeVariant {
    Default,
    Dracula,
    Gruvbox,
    Nord,
}

impl ThemeVariant {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Dracula => "Dracula",
            Self::Gruvbox => "Gruvbox",
            Self::Nord    => "Nord",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Default => Self::Dracula,
            Self::Dracula => Self::Gruvbox,
            Self::Gruvbox => Self::Nord,
            Self::Nord    => Self::Default,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "dracula" => Self::Dracula,
            "gruvbox" => Self::Gruvbox,
            "nord"    => Self::Nord,
            _         => Self::Default,
        }
    }
}

// ── Theme struct ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Theme {
    pub border:         Style,
    pub border_focused: Style,
    pub title:          Style,
    pub text:           Style,
    pub text_dim:       Style,
    pub selected:       Style,
    pub header:         Style,
    pub ok:             Style,
    pub warn:           Style,
    pub crit:           Style,
    pub read_spark:     Style,
    pub write_spark:    Style,
    pub bar_low:        Style,
    pub bar_mid:        Style,
    pub bar_high:       Style,
    pub bar_crit:       Style,
    pub footer_bg:      Style,
    pub footer_key:     Style,
    pub footer_text:    Style,
}

impl Theme {
    pub fn for_variant(v: ThemeVariant) -> Self {
        match v {
            ThemeVariant::Default => Self::default(),
            ThemeVariant::Dracula => Self::dracula(),
            ThemeVariant::Gruvbox => Self::gruvbox(),
            ThemeVariant::Nord    => Self::nord(),
        }
    }

    pub fn default() -> Self {
        Self {
            border:         Style::default().fg(Color::DarkGray),
            border_focused: Style::default().fg(Color::Cyan),
            title:          Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            text:           Style::default().fg(Color::White),
            text_dim:       Style::default().fg(Color::DarkGray),
            selected:       Style::default().fg(Color::Black).bg(Color::Cyan),
            header:         Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD),
            ok:             Style::default().fg(Color::Green),
            warn:           Style::default().fg(Color::Yellow),
            crit:           Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            read_spark:     Style::default().fg(Color::Cyan),
            write_spark:    Style::default().fg(Color::Yellow),
            bar_low:        Style::default().fg(Color::Green),
            bar_mid:        Style::default().fg(Color::Yellow),
            bar_high:       Style::default().fg(Color::LightRed),
            bar_crit:       Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            footer_bg:      Style::default().bg(Color::DarkGray).fg(Color::White),
            footer_key:     Style::default().bg(Color::DarkGray).fg(Color::Cyan).add_modifier(Modifier::BOLD),
            footer_text:    Style::default().bg(Color::DarkGray).fg(Color::Gray),
        }
    }

    fn dracula() -> Self {
        // https://draculatheme.com/ — purple/pink dark theme
        // bg: #282a36  current-line: #44475a  comment: #6272a4
        // fg: #f8f8f2  cyan: #8be9fd  green: #50fa7b  yellow: #f1fa8c
        // orange: #ffb86c  pink: #ff79c6  purple: #bd93f9  red: #ff5555
        Self {
            border:         Style::default().fg(rgb(0x6272a4)),
            border_focused: Style::default().fg(rgb(0xbd93f9)),
            title:          Style::default().fg(rgb(0xf8f8f2)).add_modifier(Modifier::BOLD),
            text:           Style::default().fg(rgb(0xf8f8f2)),
            text_dim:       Style::default().fg(rgb(0x6272a4)),
            selected:       Style::default().fg(rgb(0x282a36)).bg(rgb(0xff79c6)),
            header:         Style::default().fg(rgb(0xf8f8f2)).bg(rgb(0x44475a)).add_modifier(Modifier::BOLD),
            ok:             Style::default().fg(rgb(0x50fa7b)),
            warn:           Style::default().fg(rgb(0xf1fa8c)),
            crit:           Style::default().fg(rgb(0xff5555)).add_modifier(Modifier::BOLD),
            read_spark:     Style::default().fg(rgb(0x8be9fd)),
            write_spark:    Style::default().fg(rgb(0xffb86c)),
            bar_low:        Style::default().fg(rgb(0x50fa7b)),
            bar_mid:        Style::default().fg(rgb(0xf1fa8c)),
            bar_high:       Style::default().fg(rgb(0xffb86c)),
            bar_crit:       Style::default().fg(rgb(0xff5555)).add_modifier(Modifier::BOLD),
            footer_bg:      Style::default().bg(rgb(0x44475a)).fg(rgb(0xf8f8f2)),
            footer_key:     Style::default().bg(rgb(0x44475a)).fg(rgb(0xbd93f9)).add_modifier(Modifier::BOLD),
            footer_text:    Style::default().bg(rgb(0x44475a)).fg(rgb(0x6272a4)),
        }
    }

    fn gruvbox() -> Self {
        // https://github.com/morhetz/gruvbox — warm retro dark theme
        // bg0: #282828  bg1: #3c3836  bg3/medium: #665c54
        // fg1: #ebdbb2  fg4: #a89984
        // red: #fb4934  green: #b8bb26  yellow: #fabd2f
        // blue/aqua: #83a598  orange: #fe8019
        Self {
            border:         Style::default().fg(rgb(0x504945)),
            border_focused: Style::default().fg(rgb(0x83a598)),
            title:          Style::default().fg(rgb(0xebdbb2)).add_modifier(Modifier::BOLD),
            text:           Style::default().fg(rgb(0xebdbb2)),
            text_dim:       Style::default().fg(rgb(0xa89984)),
            selected:       Style::default().fg(rgb(0x282828)).bg(rgb(0xd79921)),
            header:         Style::default().fg(rgb(0xebdbb2)).bg(rgb(0x504945)).add_modifier(Modifier::BOLD),
            ok:             Style::default().fg(rgb(0xb8bb26)),
            warn:           Style::default().fg(rgb(0xfabd2f)),
            crit:           Style::default().fg(rgb(0xfb4934)).add_modifier(Modifier::BOLD),
            read_spark:     Style::default().fg(rgb(0x83a598)),
            write_spark:    Style::default().fg(rgb(0xfe8019)),
            bar_low:        Style::default().fg(rgb(0xb8bb26)),
            bar_mid:        Style::default().fg(rgb(0xfabd2f)),
            bar_high:       Style::default().fg(rgb(0xfe8019)),
            bar_crit:       Style::default().fg(rgb(0xfb4934)).add_modifier(Modifier::BOLD),
            footer_bg:      Style::default().bg(rgb(0x3c3836)).fg(rgb(0xebdbb2)),
            footer_key:     Style::default().bg(rgb(0x3c3836)).fg(rgb(0x83a598)).add_modifier(Modifier::BOLD),
            footer_text:    Style::default().bg(rgb(0x3c3836)).fg(rgb(0xa89984)),
        }
    }

    fn nord() -> Self {
        // https://www.nordtheme.com/ — Arctic, north-bluish clean theme
        // Polar Night: #2e3440 #3b4252 #434c5e #4c566a
        // Snow Storm:  #d8dee9 #e5e9f0 #eceff4
        // Frost:       #8fbcbb #88c0d0 #81a1c1 #5e81ac
        // Aurora:      #bf616a #d08770 #ebcb8b #a3be8c #b48ead
        Self {
            border:         Style::default().fg(rgb(0x4c566a)),
            border_focused: Style::default().fg(rgb(0x88c0d0)),
            title:          Style::default().fg(rgb(0xeceff4)).add_modifier(Modifier::BOLD),
            text:           Style::default().fg(rgb(0xe5e9f0)),
            text_dim:       Style::default().fg(rgb(0x4c566a)),
            selected:       Style::default().fg(rgb(0x2e3440)).bg(rgb(0x88c0d0)),
            header:         Style::default().fg(rgb(0xeceff4)).bg(rgb(0x3b4252)).add_modifier(Modifier::BOLD),
            ok:             Style::default().fg(rgb(0xa3be8c)),
            warn:           Style::default().fg(rgb(0xebcb8b)),
            crit:           Style::default().fg(rgb(0xbf616a)).add_modifier(Modifier::BOLD),
            read_spark:     Style::default().fg(rgb(0x88c0d0)),
            write_spark:    Style::default().fg(rgb(0xd08770)),
            bar_low:        Style::default().fg(rgb(0xa3be8c)),
            bar_mid:        Style::default().fg(rgb(0xebcb8b)),
            bar_high:       Style::default().fg(rgb(0xd08770)),
            bar_crit:       Style::default().fg(rgb(0xbf616a)).add_modifier(Modifier::BOLD),
            footer_bg:      Style::default().bg(rgb(0x3b4252)).fg(rgb(0xd8dee9)),
            footer_key:     Style::default().bg(rgb(0x3b4252)).fg(rgb(0x88c0d0)).add_modifier(Modifier::BOLD),
            footer_text:    Style::default().bg(rgb(0x3b4252)).fg(rgb(0x4c566a)),
        }
    }

    /// Pick a utilisation-gradient style for a 0–100 value.
    pub fn util_style(&self, pct: f64) -> Style {
        if      pct >= 95.0 { self.bar_crit }
        else if pct >= 75.0 { self.bar_high }
        else if pct >= 50.0 { self.bar_mid  }
        else                 { self.bar_low  }
    }
}
