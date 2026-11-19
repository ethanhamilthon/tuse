use crossterm::style::Color;
use tuse_config::Theme;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
    pub underline: bool,
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

/// Parses a theme color: `#rrggbb`, `0`-`255`, a name, or `none`.
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

/// Theme colors parsed once per (re)load.
pub struct Palette {
    pub t: Theme,
    pub accent: Option<Color>,
    pub bg: Option<Color>,
    pub fg: Option<Color>,
    pub dim: Option<Color>,
    pub user: Option<Color>,
    pub user_bg: Option<Color>,
    pub assistant: Option<Color>,
    pub reasoning: Option<Color>,
    pub tool: Option<Color>,
    pub tool_output: Option<Color>,
    pub tool_error: Option<Color>,
    pub diff_add: Option<Color>,
    pub diff_del: Option<Color>,
    pub error: Option<Color>,
    pub notice: Option<Color>,
    pub heading: Option<Color>,
    pub code: Option<Color>,
    pub code_block: Option<Color>,
    pub link: Option<Color>,
    pub quote: Option<Color>,
    pub rule: Option<Color>,
    pub status: Option<Color>,
    pub picker_fg: Option<Color>,
    pub picker_bg: Option<Color>,
    pub picker_selected_fg: Option<Color>,
    pub picker_selected_bg: Option<Color>,
    pub picker_detail: Option<Color>,
    pub picker_title: Option<Color>,
    pub input_bg: Option<Color>,
    pub bar: Option<Color>,
    pub panel_fg: Option<Color>,
    pub panel_bg: Option<Color>,
    pub panel_title: Option<Color>,
    pub motion_from: Option<Color>,
    pub motion_to: Option<Color>,
}

impl Palette {
    pub fn new(t: &Theme) -> Self {
        let color = |v: &str| {
            if v.trim() == "accent" {
                color(&t.accent)
            } else {
                color(v)
            }
        };
        Self {
            accent: color(&t.accent),
            bg: color(&t.bg),
            fg: color(&t.fg),
            dim: color(&t.dim),
            user: color(&t.user),
            user_bg: color(&t.user_bg),
            assistant: color(&t.assistant),
            reasoning: color(&t.reasoning),
            tool: color(&t.tool),
            tool_output: color(&t.tool_output),
            tool_error: color(&t.tool_error),
            diff_add: color(&t.diff_add),
            diff_del: color(&t.diff_del),
            error: color(&t.error),
            notice: color(&t.notice),
            heading: color(&t.heading),
            code: color(&t.code),
            code_block: color(&t.code_block),
            link: color(&t.link),
            quote: color(&t.quote),
            rule: color(&t.rule),
            status: color(&t.status),
            picker_fg: color(&t.picker_fg),
            picker_bg: color(&t.picker_bg),
            picker_selected_fg: color(&t.picker_selected_fg),
            picker_selected_bg: color(&t.picker_selected_bg),
            picker_detail: color(&t.picker_detail),
            picker_title: color(&t.picker_title),
            input_bg: color(&t.input_bg),
            bar: color(&t.bar),
            panel_fg: color(&t.panel_fg),
            panel_bg: color(&t.panel_bg),
            panel_title: color(&t.panel_title),
            motion_from: color(&t.motion_from),
            motion_to: color(&t.motion_to),
            t: t.clone(),
        }
    }
}
