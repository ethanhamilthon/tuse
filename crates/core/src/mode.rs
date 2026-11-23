use std::path::PathBuf;

/// Who is on the other side of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Tui,
    /// `tuse json`: a client drives the session over JSON-RPC.
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
    /// `--config PATH`; without it the config is searched for on every (re)load.
    pub config: Option<PathBuf>,
    pub mode: Mode,
    /// Resume this session instead of creating a new one.
    pub resume: Option<String>,
    /// Why the session opens, for `start`.
    pub start: Start,
}

/// The `reason` of `start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Start {
    /// Tuse was started (with or without `--resume`).
    #[default]
    Launch,
    /// `/new`.
    New,
    /// `/resume`.
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
