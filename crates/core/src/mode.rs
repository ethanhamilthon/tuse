use crate::session::Session;
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

impl Session {
    /// `ask` and `draw_ui` need someone to show them to.
    pub(crate) fn require_client(&self, what: &str) -> Result<(), String> {
        if self.mode == Mode::Headless {
            return Err(format!(
                "{what} is unavailable in headless mode (tuse -p): there is no user to answer or see it; decide yourself"
            ));
        }
        Ok(())
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
