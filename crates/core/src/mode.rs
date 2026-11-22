use std::path::PathBuf;

/// Who is on the other side of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Tui,
    /// `tuse -p`: one prompt, nobody to answer.
    Headless,
    /// `tuse json`: a client drives the session over JSON-RPC.
    Json,
}

impl From<Mode> for tuse_config::Mode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Tui => Self::Tui,
            Mode::Headless => Self::Headless,
            Mode::Json => Self::Json,
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
    /// Why the session opens, for `process_start`.
    pub start: Start,
}

/// The `reason` of `process_start`.
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
