use std::time::Duration;

/// What a session tells its front ends (TUI or headless printer).
#[derive(Debug, Clone)]
pub enum Event {
    User {
        id: crate::control::InputId,
        text: String,
        images: usize,
    },
    TurnStart,
    Text(String),
    Reasoning(String),
    /// The partial reply was dropped before a retry.
    StreamReset,
    Assistant {
        text: String,
    },
    ToolStart {
        id: String,
        tool: String,
        /// The code of `python`.
        input: String,
        title: String,
    },
    ToolEnd {
        id: String,
        output: String,
        code: Option<i32>,
        /// Files the call changed.
        diffs: Vec<crate::record::FileDiff>,
    },
    /// The working indicator has a new status.
    Status(String),
    Notice(String),
    Error(String),
    TurnEnd {
        elapsed: Duration,
        calls: usize,
        cancelled: bool,
    },
    Ask {
        id: String,
        question: String,
        options: Vec<String>,
    },
    AskDone {
        id: String,
    },
    Tasks,
    /// `draw_ui()`: a plain text block; the same `id` replaces the earlier one.
    Print {
        id: String,
        text: String,
    },
    Usage,
    /// A provider's model list arrived.
    Models,
}
