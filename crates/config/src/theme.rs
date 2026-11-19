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
    pub diff_add: String,
    pub diff_del: String,
    pub error: String,
    pub notice: String,
    pub heading: String,
    pub code: String,
    pub code_block: String,
    pub link: String,
    /// Python syntax in tool calls and in ```python blocks; the rest of the code is `tool`.
    pub syntax_keyword: String,
    pub syntax_string: String,
    pub syntax_comment: String,
    pub syntax_number: String,
    pub syntax_function: String,
    pub quote: String,
    pub rule: String,
    pub status: String,
    pub list_selected_fg: String,
    pub list_selected_bg: String,
    pub list_detail: String,
    pub list_title: String,
    pub input_bg: String,
    pub bar: String,
    pub list_fg: String,
    pub list_bg: String,
    /// The ends of the gradient that travels along the input while the agent works.
    pub motion_from: String,
    pub motion_to: String,
    // spacing
    pub pad_x: u16,
    pub message_gap: u16,
    pub list_indent: u16,
    pub code_indent: u16,
    pub list_pad_x: u16,
    pub input_pad_x: u16,
    /// Extra rows above and below the text in `bar`; the bar runs through them.
    pub input_top_padding: u16,
    pub input_bottom_padding: u16,
    /// Width of the role column in the `/trace` list.
    pub trace_role_width: u16,
    // limits
    pub list_max_height: u16,
    /// Rows the input grows to; `-1` removes the limit.
    pub input_max_height: i32,
    /// Cells per wave of the gradient, and ms per step of its animation.
    pub motion_period: u16,
    pub motion_ms: u64,
    /// Lines shown per block: `0` hides it, `-1` removes the limit.
    pub tool_line: i32,
    /// Lines of a `write` or `edit` diff shown in the chat.
    pub diff_line: i32,
    pub output_line: i32,
    pub reasoning_line: i32,
    pub scroll_step: u16,
    pub file_picker_max: usize,
    pub frame_ms: u64,
    /// `false` turns the gradient animation off.
    pub motion: bool,
    /// `lines`, `box`, `fill` or `bar`.
    pub input_design: String,
    // symbols
    pub input_prefix: String,
    pub bar_char: String,
    /// Top left, top right, bottom left, bottom right and vertical, for `box`.
    pub box_chars: Vec<String>,
    /// Half blocks above and below the input in `fill`.
    pub edge_top: String,
    pub edge_bottom: String,
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
    /// Shown instead of the status while a message you sent waits for the agent.
    pub steering_label: String,
    /// Statuses of the working indicator; `{}` is the argument.
    pub thinking_label: String,
    pub read_label: String,
    pub write_label: String,
    pub edit_label: String,
    pub grep_label: String,
    pub sh_label: String,
    pub ask_label: String,
    pub docs_label: String,
    /// The last row of an `ask`: the user writes their own answer.
    pub ask_custom_label: String,
    pub list_selected_prefix: String,
    pub more: String,
    /// Replaces each typed char of a secret in inline prompts.
    pub mask_char: String,
    pub spinner: Vec<String>,
}

impl Default for Theme {
    fn default() -> Self {
        let s = |v: &str| v.to_string();
        Self {
            accent: s("#EDEDED"),
            bg: s("#0A0A0A"),
            fg: s("#EDEDED"),
            dim: s("#A0A0A0"),
            user: s("none"),
            user_bg: s("#1A1A1A"),
            assistant: s("none"),
            reasoning: s("#676767"),
            tool: s("#52A8FF"),
            tool_output: s("#A0A0A0"),
            tool_error: s("#F32E40"),
            diff_add: s("#00AC3A"),
            diff_del: s("#F32E40"),
            error: s("#F32E40"),
            notice: s("#878787"),
            heading: s("#52A8FF"),
            code: s("#00AC3A"),
            code_block: s("#EDEDED"),
            link: s("#52A8FF"),
            syntax_keyword: s("#F75F8F"),
            syntax_string: s("#6BD968"),
            syntax_comment: s("#676767"),
            syntax_number: s("#FFB224"),
            syntax_function: s("#BF7AF0"),
            quote: s("#A0A0A0"),
            rule: s("#2E2E2E"),
            status: s("accent"),
            list_selected_fg: s("#0A0A0A"),
            list_selected_bg: s("accent"),
            list_detail: s("#A0A0A0"),
            list_title: s("accent"),
            input_bg: s("#1A1A1A"),
            bar: s("accent"),
            list_fg: s("none"),
            list_bg: s("#000000"),
            motion_from: s("#676767"),
            motion_to: s("accent"),
            pad_x: 0,
            message_gap: 1,
            list_indent: 2,
            code_indent: 2,
            list_pad_x: 0,
            input_pad_x: 0,
            input_top_padding: 0,
            input_bottom_padding: 0,
            trace_role_width: 9,
            list_max_height: 16,
            input_max_height: -1,
            motion_period: 24,
            motion_ms: 60,
            tool_line: -1,
            diff_line: 12,
            output_line: 1,
            reasoning_line: -1,
            scroll_step: 3,
            file_picker_max: 20000,
            frame_ms: 33,
            motion: true,
            input_design: s("lines"),
            input_prefix: s("› "),
            bar_char: s("┃"),
            box_chars: ["╭", "╮", "╰", "╯", "│"].map(String::from).to_vec(),
            edge_top: s("▄"),
            edge_bottom: s("▀"),
            user_prefix: s("› "),
            tool_prefix: s("* "),
            reasoning_prefix: s("∴ "),
            notice_prefix: s("✓ "),
            error_prefix: s("✗ "),
            bullet: s("• "),
            quote_prefix: s("│ "),
            rule_char: s("─"),
            table_sep: s(" │ "),
            scroll_mark: s("↑"),
            label_sep: s(" · "),
            steering_label: s("steering…"),
            thinking_label: s("thinking..."),
            read_label: s("reading {}"),
            write_label: s("writing {}"),
            edit_label: s("editing {}"),
            grep_label: s("searching {}"),
            sh_label: s("running {}"),
            ask_label: s("asking user..."),
            docs_label: s("reading docs..."),
            ask_custom_label: s("Write your own answer…"),
            list_selected_prefix: s("› "),
            more: s("…"),
            mask_char: s("•"),
            spinner: ["◐", "◓", "◑", "◒"].map(String::from).to_vec(),
        }
    }
}

impl Theme {
    pub fn check(&self) -> Result<(), String> {
        if !matches!(self.input_design.as_str(), "lines" | "box" | "fill" | "bar") {
            return Err(format!(
                "input_design `{}` is not lines, box, fill or bar",
                self.input_design
            ));
        }
        if self.box_chars.len() != 5 {
            return Err(
                "box_chars needs 5 items: top left, top right, bottom left, bottom right, vertical"
                    .into(),
            );
        }
        Ok(())
    }
}

/// How many of `len` lines a `*_line` setting shows.
pub fn limit(setting: i32, len: usize) -> usize {
    usize::try_from(setting).map_or(len, |n| n.min(len))
}
