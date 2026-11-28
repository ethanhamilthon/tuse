use serde::{Deserialize, Serialize};
use tuse_api::{Replay, ToolCall, Usage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Flash,
    Notice,
    Error,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Flash => "flash",
            Level::Notice => "notice",
            Level::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub code: String,
    pub status: String,
    pub ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskMessage {
    pub id: String,
    pub name: String,
    pub status: String,
    pub delivery: TaskDelivery,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDelivery {
    Wake,
    Steer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    System {
        text: String,
    },
    User {
        text: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        context: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
    },
    Agent {
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
    Lua {
        id: String,
        code: String,
        output: String,
        #[serde(default)]
        exit: Option<i32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        diffs: Vec<FileDiff>,
    },
    Notification {
        text: String,
    },
    TaskMessage(TaskMessage),
    Message {
        level: Level,
        text: String,
    },
    Usage {
        model: String,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_write: u64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        llm: bool,
    },
    Meta {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
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
    Show {
        name: String,
        text: String,
        pin: bool,
    },
    Pick {
        id: u64,
        prompt: String,
        value: String,
    },
    Ask {
        id: u64,
        prompt: String,
        value: String,
    },
    Task(JobRecord),
    Command(JobRecord),
    Open(JobRecord),
}

impl Record {
    pub fn usage(model: String, u: Usage, llm: bool) -> Self {
        Record::Usage {
            model,
            input: u.input,
            output: u.output,
            cache_read: u.cache_read,
            cache_write: u.cache_write,
            llm,
        }
    }

    pub fn message(level: Level, text: impl Into<String>) -> Self {
        Record::Message {
            level,
            text: text.into(),
        }
    }

    pub(crate) fn job(
        kind: &str,
        name: String,
        code: String,
        status: String,
        ms: u64,
        error: Option<String>,
    ) -> Self {
        let j = JobRecord {
            id: None,
            name,
            code,
            status,
            ms,
            error,
        };
        match kind {
            "task" => Record::Task(j),
            "command" => Record::Command(j),
            _ => Record::Open(j),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub diff: String,
}

#[derive(Debug, Clone, Default)]
pub struct UserInput {
    pub text: String,
    pub context: String,
    pub images: Vec<String>,
}

impl UserInput {
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            text: t.into(),
            ..Self::default()
        }
    }
}
