use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct Request {
    pub model: String,
    pub messages: Vec<Value>,
    pub tools: Vec<Value>,
    /// `None` sends no `reasoning_effort` at all.
    pub effort: Option<String>,
}

/// Token counts of one request. `input` includes cached and cache-write tokens.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
    pub cache_write: u64,
}

impl Usage {
    pub fn add(&mut self, o: &Usage) {
        self.input += o.input;
        self.output += o.output;
        self.cached += o.cached;
        self.cache_write += o.cache_write;
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Default)]
pub struct Reply {
    pub text: String,
    pub reasoning: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
    pub finish: String,
}

pub enum Delta<'a> {
    Tool {
        index: usize,
        id: &'a str,
        name: &'a str,
        code: &'a str,
        title: Option<&'a str>,
    },
    Text(&'a str),
    Reasoning(&'a str),
}

#[derive(Debug, Clone)]
pub enum Error {
    Status(u16, String),
    Transport(String),
    Parse(String),
    Cancelled,
}

impl Error {
    pub fn retryable(&self) -> bool {
        match self {
            Error::Status(code, _) => *code == 429 || *code >= 500,
            Error::Transport(_) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Status(code, body) => write!(f, "HTTP {code}: {}", one_line(body)),
            Error::Transport(e) => write!(f, "network: {e}"),
            Error::Parse(e) => write!(f, "bad response: {e}"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

fn one_line(s: &str) -> String {
    let s = s.trim().replace('\n', " ");
    match s.char_indices().nth(300) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn rate_limits_server_errors_and_the_network_are_retried() {
        for (code, retry) in [
            (429, true),
            (500, true),
            (503, true),
            (400, false),
            (401, false),
        ] {
            assert_eq!(
                Error::Status(code, String::new()).retryable(),
                retry,
                "{code}"
            );
        }
        assert!(Error::Transport("reset".into()).retryable());
        assert!(!Error::Parse("x".into()).retryable() && !Error::Cancelled.retryable());
    }
}
