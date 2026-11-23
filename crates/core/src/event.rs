use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiPrompt {
    Picker { options: Vec<String> },
    Question { text: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UiRequest {
    pub id: u64,
    #[serde(flatten)]
    pub prompt: UiPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UiMarkdown {
    pub name: String,
    pub text: String,
    pub pin: bool,
    #[serde(skip)]
    pub after_record: usize,
    #[serde(skip)]
    pub seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UiEntry {
    pub id: u64,
    pub kind: &'static str,
    pub prompt: String,
    pub value: String,
    #[serde(skip)]
    pub after_record: usize,
    #[serde(skip)]
    pub seq: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiSnapshot {
    pub markdowns: Vec<UiMarkdown>,
    pub active: Option<UiRequest>,
    pub entries: Vec<UiEntry>,
}

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
        /// The code of the `lua` call.
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
    /// A reload committed a new config (from `/reload`, `config.reload` or Lua `reload()`).
    Reloaded,
    /// A named Markdown block was created, updated, or removed (`removed = text.is_empty() && !pin`).
    UiMarkdown {
        name: String,
        text: String,
        pin: bool,
        removed: bool,
    },
    /// The active interactive picker or question prompt changed.
    UiRequest(UiRequest),
    /// The active interactive prompt was cancelled externally.
    UiClear {
        id: u64,
    },
    /// A picker or question was answered (not cancelled).
    UiResult(UiEntry),
}
