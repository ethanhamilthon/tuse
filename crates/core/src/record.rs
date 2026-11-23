use serde::{Deserialize, Serialize};
use tuse_api::{ToolCall, Usage};

/// One line of `session.jsonl`.
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
        /// `data:` URLs the model saw with the output (images from `read`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
        /// Files the call changed, with their diffs.
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
    },
    Settings {
        model: String,
        effort: String,
    },
    /// One run of a hook: shown in `/trace`, never sent to the model.
    Hook {
        name: String,
        event: String,
        /// The event's own variables, `name=value` per line (long values are cut).
        vars: String,
        /// `None` when it succeeded, else the last line of the error.
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

/// A file a call changed and what changed in it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub diff: String,
}

/// A message on its way into the history: typed in the TUI, a prompt or a notification.
#[derive(Debug, Clone, Default)]
pub struct UserInput {
    pub notification: bool,
    pub text: String,
    /// Hidden XML blocks (prompts, files, images) sent to the model but not shown.
    pub context: String,
    /// `data:` URLs.
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
