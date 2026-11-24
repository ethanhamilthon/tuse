use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ApiType {
    #[default]
    OpenAiChat,
    OpenAiResponses,
    AnthropicMessages,
    GoogleGemini,
}

impl ApiType {
    pub const ALL: [&'static str; 4] = [
        "openai-chat",
        "openai-responses",
        "anthropic-messages",
        "google-gemini",
    ];

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "openai-chat" => Ok(Self::OpenAiChat),
            "openai-responses" => Ok(Self::OpenAiResponses),
            "anthropic-messages" => Ok(Self::AnthropicMessages),
            "google-gemini" => Ok(Self::GoogleGemini),
            _ => Err(format!(
                "invalid --api `{s}` (supported: {})",
                Self::ALL.join(", ")
            )),
        }
    }

    pub fn from_stored(s: &str) -> Self {
        match s.trim() {
            "" => Self::OpenAiChat,
            other => Self::parse(other).unwrap_or(Self::OpenAiChat),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiChat => "openai-chat",
            Self::OpenAiResponses => "openai-responses",
            Self::AnthropicMessages => "anthropic-messages",
            Self::GoogleGemini => "google-gemini",
        }
    }

    pub fn supported_efforts(self) -> &'static [&'static str] {
        match self {
            Self::OpenAiChat => &["default", "low", "medium", "high", "xhigh", "max"],
            Self::OpenAiResponses => &["default", "low", "medium", "high", "xhigh"],
            Self::AnthropicMessages => &["default", "low", "medium", "high", "max"],
            Self::GoogleGemini => &["default", "low", "medium", "high"],
        }
    }

    pub fn supports_effort(self, effort: &str) -> bool {
        self.supported_efforts().contains(&effort)
    }
}

#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub struct ToolImage {
    pub label: String,
    pub urls: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    User {
        text: String,
        images: Vec<String>,
    },
    Assistant {
        text: String,
        tool_calls: Vec<ToolCall>,
        replay: Option<Replay>,
    },
    ToolResult {
        id: String,
        name: String,
        output: String,
        images: Option<ToolImage>,
    },
}

#[derive(Debug, Clone)]
pub struct Request {
    pub provider: String,
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Replay {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    pub api: String,
    pub model: String,
    pub items: Vec<Value>,
}

impl Replay {
    pub fn matches(&self, provider: &str, api: ApiType, model: &str) -> bool {
        !self.provider.is_empty()
            && self.provider == provider
            && self.api == api.as_str()
            && self.model == model
            && !self.items.is_empty()
    }
}

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
    pub replay: Option<Replay>,
    pub usage: Option<Usage>,
    pub finish: String,
}

pub enum Delta<'a> {
    Tool {
        index: usize,
        id: &'a str,
        name: &'a str,
        code: &'a str,
    },
    Text(&'a str),
    Reasoning(&'a str),
}

#[derive(Debug, Clone)]
pub enum Error {
    Status(u16, String),
    Transport(String),
    Parse(String),
    Unsupported(String),
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
            Error::Unsupported(e) => write!(f, "{e}"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

pub(crate) fn one_line(s: &str) -> String {
    let s = s.trim().replace('\n', " ");
    match s.char_indices().nth(300) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s,
    }
}
