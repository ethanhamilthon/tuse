use crossterm::style::Color;
use tuse_config::{Config, Theme, UiSettings};

pub const FRAME_MS: u64 = 33;
/// The least time an action stays in the working indicator.
pub const STATUS_MIN_MS: u64 = 100;
pub const MOTION_PERIOD: u16 = 24;
pub const MOTION_MS: u64 = 60;
pub const FILE_PICKER_MAX: usize = 20_000;
pub const LIST_INDENT: usize = 2;
pub const CODE_INDENT: usize = 2;
pub const TRACE_ROLE_WIDTH: usize = 9;

pub const INPUT_PREFIX: &str = "› ";
pub const USER_PREFIX: &str = "› ";
pub const TOOL_PREFIX: &str = "* ";
pub const REASONING_PREFIX: &str = "∴ ";
pub const NOTICE_PREFIX: &str = "✓ ";
pub const ERROR_PREFIX: &str = "✗ ";
pub const LIST_SELECTED_PREFIX: &str = "› ";
pub const QUOTE_PREFIX: &str = "│ ";
pub const BULLET: &str = "• ";
pub const RULE_CHAR: &str = "─";
pub const TABLE_SEP: &str = " │ ";
pub const SCROLL_MARK: &str = "↑";
pub const LABEL_SEP: &str = " · ";
pub const MORE: &str = "…";
pub const MASK_CHAR: &str = "•";
pub const BAR_CHAR: &str = "┃";
pub const BOX_CHARS: [&str; 5] = ["╭", "╮", "╰", "╯", "│"];
pub const EDGE_TOP: &str = "▄";
pub const EDGE_BOTTOM: &str = "▀";
pub const SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];
pub const STEERING_LABEL: &str = "steering…";

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
    pub underline: bool,
    /// A clickable URL, interned by `links::intern`.
    pub link: Option<u32>,
}

impl Style {
    pub fn fg(c: Option<Color>) -> Self {
        Self {
            fg: c,
            ..Self::default()
        }
    }
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub fn on(mut self, bg: Option<Color>) -> Self {
        self.bg = bg;
        self
    }
}

/// Parses a theme color: `#rrggbb`, `#rgb`, `0`-`255`, a name, or `none`.
pub fn color(s: &str) -> Option<Color> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        let v = u32::from_str_radix(hex, 16).ok()?;
        return match hex.len() {
            6 => Some(Color::Rgb {
                r: (v >> 16) as u8,
                g: (v >> 8) as u8,
                b: v as u8,
            }),
            3 => Some(Color::Rgb {
                r: ((v >> 8) & 0xf) as u8 * 17,
                g: ((v >> 4) & 0xf) as u8 * 17,
                b: (v & 0xf) as u8 * 17,
            }),
            _ => None,
        };
    }
    if let Ok(n) = s.parse::<u8>() {
        return Some(Color::AnsiValue(n));
    }
    Some(match s.as_str() {
        "black" => Color::Black,
        "red" => Color::DarkRed,
        "green" => Color::DarkGreen,
        "yellow" => Color::DarkYellow,
        "blue" => Color::DarkBlue,
        "magenta" => Color::DarkMagenta,
        "cyan" => Color::DarkCyan,
        "white" | "grey" | "gray" => Color::Grey,
        "dark_grey" | "dark_gray" => Color::DarkGrey,
        "bright_red" => Color::Red,
        "bright_green" => Color::Green,
        "bright_yellow" => Color::Yellow,
        "bright_blue" => Color::Blue,
        "bright_magenta" => Color::Magenta,
        "bright_cyan" => Color::Cyan,
        "bright_white" => Color::White,
        _ => return None,
    })
}

/// The 10 theme colors parsed once per (re)load, plus `settings.ui`.
pub struct Palette {
    pub ui: UiSettings,
    pub bg: Option<Color>,
    pub surface: Option<Color>,
    pub fg: Option<Color>,
    pub dim: Option<Color>,
    pub muted: Option<Color>,
    pub border: Option<Color>,
    pub accent: Option<Color>,
    pub ok: Option<Color>,
    pub err: Option<Color>,
    pub info: Option<Color>,
}

impl Palette {
    pub fn new(t: &Theme, ui: &UiSettings) -> Self {
        let c = |v: &str| {
            if v.trim() == "accent" {
                color(&t.accent)
            } else {
                color(v)
            }
        };
        Self {
            ui: ui.clone(),
            bg: c(&t.bg),
            surface: c(&t.surface),
            fg: c(&t.fg),
            dim: c(&t.dim),
            muted: c(&t.muted),
            border: c(&t.border),
            accent: c(&t.accent),
            ok: c(&t.ok),
            err: c(&t.err),
            info: c(&t.info),
        }
    }

    pub fn from_config(cfg: &Config) -> Self {
        Self::new(&cfg.active_theme(), &cfg.settings.ui)
    }
}

pub const FLASH_SECS: u64 = 5;
