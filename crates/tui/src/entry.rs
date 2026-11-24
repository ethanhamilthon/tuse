use std::time::Duration;
use tuse_core::FileDiff;

#[derive(Debug, Clone)]
pub enum Entry {
    User {
        text: String,
        images: usize,
    },
    Assistant(String),
    Reasoning(String),
    Tool {
        id: String,
        input: String,
        output: Option<String>,
        code: Option<i32>,
        diffs: Vec<FileDiff>,
    },
    Notice(String),
    Error(String),
    TurnEnd {
        elapsed: Duration,
        model: String,
        effort: String,
        cancelled: bool,
    },
    Markdown {
        name: String,
        text: String,
    },
    UiResult {
        is_picker: bool,
        prompt: String,
        value: String,
    },
}
