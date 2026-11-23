use std::time::Duration;

/// What a session tells its front ends (TUI or JSON-RPC).
#[derive(Debug, Clone)]
pub enum Event {
    Notification(String),
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
    ToolDelta {
        index: usize,
        id: String,
        name: String,
        code: String,
    },
    ToolStart {
        timeout: u64,
        id: String,
        tool: String,
        /// The code of `python`.
        input: String,
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
    Flash(String),
    Error(String),
    TurnEnd {
        elapsed: Duration,
        calls: usize,
        model: String,
        effort: String,
        cancelled: bool,
    },
    Tasks,
    Usage,
    /// A provider's model list arrived.
    Models,
}
