use serde::Deserialize;

/// The 10 base colors of a theme. Values are `#rrggbb`, `#rgb`, an ANSI number
/// (`0`-`255`), a name (`red`, `dark_grey`, ...), `accent`, or `none`.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
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

/// User-facing UI behavior and layout preferences (`settings.ui`), independent of the color theme.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
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
