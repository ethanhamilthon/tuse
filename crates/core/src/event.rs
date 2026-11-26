use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiPrompt {
    Picker {
        options: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Question {
        text: String,
    },
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UiEntry {
    pub id: u64,
    pub kind: &'static str,
    pub prompt: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiSnapshot {
    pub markdowns: Vec<UiMarkdown>,
    pub active: Option<UiRequest>,
    pub entries: Vec<UiEntry>,
}

#[derive(Debug, Clone)]
pub enum Event {
    Notification(String),
    User {
        id: InputId,
        text: String,
        images: usize,
    },
    TurnStart,
    Text(String),
    Reasoning(String),
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
        input: String,
    },
    ToolEnd {
        id: String,
        output: String,
        code: Option<i32>,
        diffs: Vec<crate::convo::record::FileDiff>,
    },
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
    Models,
    Reloaded,
    HistoryReplaced,
    UiMarkdown {
        name: String,
        text: String,
        pin: bool,
        removed: bool,
    },
    UiRequest(UiRequest),
    UiClear {
        id: u64,
    },
    UiResult(UiEntry),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputId(pub(crate) u64);

impl InputId {
    pub fn number(self) -> u64 {
        self.0
    }
    pub fn from_number(n: u64) -> Self {
        InputId(n)
    }
}
