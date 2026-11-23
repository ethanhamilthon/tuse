use serde::{Deserialize, Serialize};
use tuse_api::{ToolCall, Usage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Record {
    User {
        text: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        context: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
        #[serde(default)]
        at: String,
    },
    Assistant {
        text: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        reasoning: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
        #[serde(default)]
        at: String,
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
        #[serde(default)]
        at: String,
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
    Compaction {
        prune: usize,
        drop: usize,
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
        vars: String,
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
