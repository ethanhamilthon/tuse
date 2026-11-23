use serde::Deserialize;

/// The 10 base colors of a theme. Values are `#rrggbb`, `#rgb`, an ANSI number
/// (`0`-`255`), a name (`red`, `dark_grey`, ...), `accent`, or `none`.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub bg: String,
    pub surface: String,
    pub fg: String,
    pub dim: String,
    pub muted: String,
    pub border: String,
    pub accent: String,
    pub ok: String,
    pub err: String,
    pub info: String,
}

impl Default for Theme {
    fn default() -> Self {
        let s = |v: &str| v.to_string();
        Self {
            bg: s("#121212"),
            surface: s("#1E1E1E"),
            fg: s("#DEDEDE"),
            dim: s("#BDBDBD"),
            muted: s("#929292"),
            border: s("#333333"),
            accent: s("#BDDEAB"),
            ok: s("#94B979"),
            err: s("#E27373"),
            info: s("#97BEDC"),
        }
    }
}

/// Whether `s` is a valid theme color token (`#rrggbb`, `#rgb`, `0`-`255`, named color, `accent`, `none`).
pub fn valid_color(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    if matches!(
        s.as_str(),
        "none"
            | "accent"
            | "black"
            | "red"
            | "green"
            | "yellow"
            | "blue"
            | "magenta"
            | "cyan"
            | "white"
            | "grey"
            | "gray"
            | "dark_grey"
            | "dark_gray"
            | "bright_red"
            | "bright_green"
            | "bright_yellow"
            | "bright_blue"
            | "bright_magenta"
            | "bright_cyan"
            | "bright_white"
    ) {
        return true;
    }
    if s.parse::<u8>().is_ok() {
        return true;
    }
    if let Some(hex) = s.strip_prefix('#') {
        return matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    false
}

/// User-facing UI behavior and layout preferences (`settings.ui`), independent of the color theme.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiSettings {
    /// `true` folds multiline Lua long strings (`[[...]]`) in chat tool calls.
    pub fold_strings: bool,
    /// Lines shown per block: `0` hides it, `-1` removes the limit.
    pub tool_line: i32,
    pub diff_line: i32,
    pub output_line: i32,
    pub reasoning_line: i32,
    pub scroll_step: u16,
    pub pad_x: u16,
    pub message_gap: u16,
    pub list_max_height: usize,
    /// Rows the input grows to; `-1` removes the limit.
    pub input_max_height: i32,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            fold_strings: true,
            tool_line: -1,
            diff_line: 12,
            output_line: 1,
            reasoning_line: -1,
            scroll_step: 3,
            pad_x: 0,
            message_gap: 1,
            list_max_height: 16,
            input_max_height: -1,
        }
    }
}

/// How many of `len` lines a `*_line` setting shows.
pub fn limit(setting: i32, len: usize) -> usize {
    usize::try_from(setting).map_or(len, |n| n.min(len))
}
