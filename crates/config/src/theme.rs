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
            bg: s("#0A0A0A"),
            surface: s("#1A1A1A"),
            fg: s("#EDEDED"),
            dim: s("#A0A0A0"),
            muted: s("#676767"),
            border: s("#2E2E2E"),
            accent: s("#EDEDED"),
            ok: s("#00AC3A"),
            err: s("#F32E40"),
            info: s("#52A8FF"),
        }
    }
}

/// User-facing UI behavior and layout preferences (`settings.ui`), independent of the color theme.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct UiSettings {
    pub osc_status: bool,
    /// `lines`, `box`, `fill` or `bar`.
    pub input_design: String,
    /// `false` turns the working gradient animation off.
    pub motion: bool,
    /// `true` folds multiline triple-quoted Python strings in chat tool calls.
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
            input_design: "lines".into(),
            osc_status: true,
            motion: true,
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

impl UiSettings {
    pub fn check(&self) -> Result<(), String> {
        if !matches!(self.input_design.as_str(), "lines" | "box" | "fill" | "bar") {
            return Err(format!(
                "input_design `{}` is not lines, box, fill or bar",
                self.input_design
            ));
        }
        Ok(())
    }
}

/// How many of `len` lines a `*_line` setting shows.
pub fn limit(setting: i32, len: usize) -> usize {
    usize::try_from(setting).map_or(len, |n| n.min(len))
}
