mod diff;
mod edit;
mod fs;
mod read;
mod write;

use serde_json::Value;
use std::path::{Path, PathBuf};

pub use fs::resolve;

/// The result of one tool call.
pub struct Output {
    pub text: String,
    pub ok: bool,
    /// `data:` URLs the model should see next to the text (images from `read`).
    pub images: Vec<String>,
}

impl Output {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ok: true,
            images: vec![],
        }
    }

    pub fn err(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ok: false,
            images: vec![],
        }
    }
}

pub const NAMES: [&str; 4] = ["python", "read", "write", "edit"];

/// What a call shows as its input: the code of `python`, the path of the others.
pub fn input_of(tool: &str, args: &Value, raw: &str) -> String {
    let key = if tool == "python" { "code" } else { "path" };
    args[key]
        .as_str()
        .map_or_else(|| raw.to_string(), String::from)
}

/// Runs `read`, `write` or `edit`.
pub fn run(tool: &str, args: &Value, cwd: &Path, config: &tuse_config::Config) -> Output {
    let Some(path) = args["path"].as_str().filter(|p| !p.is_empty()) else {
        return Output::err("missing argument `path`");
    };
    let path: PathBuf = resolve(path, cwd);
    match tool {
        "read" => read::run(&path, args, config),
        "write" => write::run(&path, args),
        "edit" => edit::run(&path, args),
        _ => Output::err(format!("unknown tool `{tool}`")),
    }
}

pub(crate) fn text_arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, Output> {
    args[name]
        .as_str()
        .ok_or_else(|| Output::err(format!("missing argument `{name}` (a string)")))
}
