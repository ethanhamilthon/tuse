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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoadFlags {
    pub no_global: bool,
    pub no_plugins: bool,
}

pub struct Options {
    pub cwd: PathBuf,
    pub mode: Mode,
    pub resume: Option<String>,
    pub start: Start,
    pub flags: LoadFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Start {
    #[default]
    Launch,
    New,
    Resume,
}
