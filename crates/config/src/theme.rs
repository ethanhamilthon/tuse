use serde::Deserialize;

/// Every visual decision of the TUI. Colors are `#rrggbb`, an ANSI number
/// (`0`-`255`), a name (`red`, `dark_grey`, ...), `accent` for the accent color,
/// or `none` for the terminal default.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Theme {
    // colors
    pub accent: String,
    pub bg: String,
    pub fg: String,
    pub dim: String,
    pub user: String,
    pub user_bg: String,
    pub assistant: String,
    pub reasoning: String,
    pub tool: String,
    pub tool_output: String,
    pub tool_error: String,
    pub error: String,
    pub notice: String,
    pub heading: String,
    pub code: String,
    pub code_block: String,
    pub link: String,
    pub quote: String,
    pub rule: String,
    pub status: String,
    pub picker_fg: String,
    pub picker_bg: String,
    pub picker_selected_fg: String,
    pub picker_selected_bg: String,
    pub picker_detail: String,
    pub picker_title: String,
    // spacing
    pub pad_x: u16,
    pub message_gap: u16,
    pub list_indent: u16,
    pub code_indent: u16,
    pub picker_pad_x: u16,
    // limits
    pub picker_max_height: u16,
    pub input_max_height: u16,
    /// Lines shown per block: `0` hides it, `-1` removes the limit.
    pub bash_line: i32,
    pub output_line: i32,
    pub reasoning_line: i32,
    pub scroll_step: u16,
    pub file_picker_max: usize,
    pub frame_ms: u64,
    // symbols
    pub user_prefix: String,
    pub tool_prefix: String,
    pub reasoning_prefix: String,
    pub notice_prefix: String,
    pub error_prefix: String,
    pub bullet: String,
    pub quote_prefix: String,
    pub rule_char: String,
    pub table_sep: String,
    pub scroll_mark: String,
    pub label_sep: String,
    pub picker_selected_prefix: String,
    pub more: String,
    /// Replaces each typed char of a secret in inline prompts.
    pub mask_char: String,
    pub spinner: Vec<String>,
}

impl Default for Theme {
    fn default() -> Self {
        let s = |v: &str| v.to_string();
        Self {
            accent: s("#E6C384"),
            bg: s("none"),
            fg: s("none"),
            dim: s("#7A7A7A"),
            user: s("none"),
            user_bg: s("#1E1E1E"),
            assistant: s("none"),
            reasoning: s("#6A6A6A"),
            tool: s("#8AB4F8"),
            tool_output: s("#8A8A8A"),
            tool_error: s("#E06C75"),
            error: s("#E06C75"),
            notice: s("#7A7A7A"),
            heading: s("accent"),
            code: s("#98C379"),
            code_block: s("#ABB2BF"),
            link: s("#61AFEF"),
            quote: s("#7A7A7A"),
            rule: s("accent"),
            status: s("accent"),
            picker_fg: s("none"),
            picker_bg: s("#1A1A1A"),
            picker_selected_fg: s("#0A0A0A"),
            picker_selected_bg: s("accent"),
            picker_detail: s("#7A7A7A"),
            picker_title: s("accent"),
            pad_x: 1,
            message_gap: 1,
            list_indent: 2,
            code_indent: 2,
            picker_pad_x: 1,
            picker_max_height: 8,
            input_max_height: 10,
            bash_line: 3,
            output_line: 3,
            reasoning_line: 2,
            scroll_step: 3,
            file_picker_max: 20000,
            frame_ms: 33,
            user_prefix: s("› "),
            tool_prefix: s("$ "),
            reasoning_prefix: s("∴ "),
            notice_prefix: s("· "),
            error_prefix: s("✗ "),
            bullet: s("• "),
            quote_prefix: s("│ "),
            rule_char: s("─"),
            table_sep: s(" │ "),
            scroll_mark: s("↑"),
            label_sep: s(" · "),
            picker_selected_prefix: s("› "),
            more: s("…"),
            mask_char: s("•"),
            spinner: ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
                .map(String::from)
                .to_vec(),
        }
    }
}

/// How many of `len` lines a `*_line` setting shows.
pub fn limit(setting: i32, len: usize) -> usize {
    usize::try_from(setting).map_or(len, |n| n.min(len))
}
