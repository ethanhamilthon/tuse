use serde::{Deserialize, Serialize};
use tuse_api::{ToolCall, Usage};

/// One line of `session.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Record {
    User {
        text: String,
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
        /// Sessions before 0.4 have no tool name: they ran `bash`.
        #[serde(default = "bash_name")]
        tool: String,
        #[serde(alias = "command")]
        input: String,
        output: String,
        #[serde(default)]
        code: Option<i32>,
        /// `data:` URLs the model saw with the output (images from `read`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
        #[serde(default)]
        at: String,
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
    },
    Settings {
        model: String,
        effort: String,
    },
    TurnEnd {
        elapsed_ms: u64,
        calls: usize,
        #[serde(default)]
        cancelled: bool,
    },
}

/// A message on its way into the history: typed in the TUI, a prompt or a notification.
#[derive(Debug, Clone, Default)]
pub struct UserInput {
    pub text: String,
    /// `data:` URLs.
    pub images: Vec<String>,
}

impl UserInput {
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            text: t.into(),
            images: vec![],
        }
    }
}

fn bash_name() -> String {
    "bash".into()
}
