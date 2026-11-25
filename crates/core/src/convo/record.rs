use serde::{Deserialize, Serialize};
use tuse_api::{Replay, ToolCall, Usage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    System {
        text: String,
    },
    Flash {
        text: String,
    },
    User {
        text: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        context: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
    },
    Assistant {
        text: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        reasoning: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replay: Option<Replay>,
        #[serde(default)]
        cancelled: bool,
    },
    Tool {
        id: String,
        tool: String,
        input: String,
        output: String,
        #[serde(default)]
        code: Option<i32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        diffs: Vec<FileDiff>,
    },
    Notification {
        text: String,
    },
    Notice {
        text: String,
    },
    Error {
        text: String,
    },
    Usage {
        model: String,
        usage: Usage,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        llm: bool,
    },
    Settings {
        model: String,
        effort: String,
    },
    Hook {
        name: String,
        event: String,
        vars: serde_json::Value,
        error: Option<String>,
        ms: u64,
    },
    TurnEnd {
        elapsed_ms: u64,
        calls: usize,
        #[serde(default)]
        model: String,
        #[serde(default)]
        effort: String,
        #[serde(default)]
        cancelled: bool,
    },
    UiMarkdown {
        name: String,
        text: String,
        pin: bool,
        removed: bool,
    },
    UiResult {
        id: u64,
        kind: String,
        prompt: String,
        value: String,
    },
    Job {
        kind: String,
        name: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        code: String,
        status: String,
        ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Title {
        new: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub diff: String,
}

#[derive(Debug, Clone, Default)]
pub struct UserInput {
    pub notification: bool,
    pub text: String,
    pub context: String,
    pub images: Vec<String>,
}

impl UserInput {
    pub fn notification(t: impl Into<String>) -> Self {
        Self {
            notification: true,
            ..Self::text(t)
        }
    }
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            text: t.into(),
            ..Self::default()
        }
    }
}
