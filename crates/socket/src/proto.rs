use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Steer {
        text: String,
        #[serde(default)]
        files: Vec<String>,
    },
    Ask {
        question: String,
        options: Vec<String>,
        #[serde(default, rename = "async")]
        is_async: bool,
    },
    TaskRun {
        command: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        cwd: Option<String>,
    },
    TaskStop {
        id: String,
    },
    TaskGet {
        id: String,
        #[serde(default)]
        lines: Option<usize>,
    },
    TaskList,
    Print {
        text: String,
        #[serde(default)]
        id: Option<String>,
    },
    Status,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub data: Value,
}

impl Response {
    pub fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data,
            ..Default::default()
        }
    }

    pub fn err(e: impl ToString) -> Self {
        Self {
            ok: false,
            error: e.to_string(),
            ..Default::default()
        }
    }
}
