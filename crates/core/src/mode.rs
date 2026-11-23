use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Tui,
    Json,
}

impl Mode {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Tui => "tui",
            Self::Json => "json",
        }
    }
}

pub struct Options {
    pub cwd: PathBuf,
    pub config: Option<PathBuf>,
    pub mode: Mode,
    pub resume: Option<String>,
    pub start: Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Start {
    #[default]
    Launch,
    New,
    Resume,
}

impl Start {
    pub fn reason(self) -> &'static str {
        match self {
            Start::Launch => "launch",
            Start::New => "new",
            Start::Resume => "resume",
        }
    }
}
