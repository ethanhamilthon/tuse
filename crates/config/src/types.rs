use crate::theme::{Theme, UiSettings};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Providers come from the database (`tuse connect`), never from the file.
    #[serde(skip)]
    pub connect: Vec<Connect>,
    pub settings: Settings,
    /// The 10 colors of the UI.
    pub theme: Theme,
    pub models: Vec<ModelDef>,
    pub systems: Vec<String>,
    pub prompts: BTreeMap<String, String>,
    pub functions: BTreeMap<String, Function>,
    pub hooks: BTreeMap<String, Hook>,
    pub commands: Vec<String>,
    /// File extensions (without leading dot) handled by `open` callbacks.
    pub open: Vec<String>,
    /// The config file, empty when there is none; filled by the loader.
    #[serde(skip_deserializing)]
    pub path: String,
    /// The loaded plugins; filled by the loader, never read from the file.
    #[serde(skip_deserializing)]
    pub plugins: Vec<PluginInfo>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PluginInfo {
    pub name: String,
    /// The plugin's folder.
    pub dir: String,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    /// Short commit of the clone, `None` for a local folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Connect {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Unset: 60% of the model's `window` when it is known; `0` turns compaction off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_at: Option<u64>,
    pub max_image_bytes: usize,
    pub retries: usize,
    pub ui: UiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            compact_at: None,
            max_image_bytes: 5 * 1024 * 1024,
            retries: 5,
            ui: UiSettings::default(),
        }
    }
}

/// Metadata for a configured `provider/model`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ModelDef {
    pub name: String,
    pub display: String,
    pub window: u64,
    pub prices: Option<Prices>,
    pub compact_at: Option<u64>,
}

/// Prices in dollars per 1M tokens.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Prices {
    #[serde(alias = "in")]
    pub input: f64,
    #[serde(alias = "out")]
    pub output: f64,
    #[serde(alias = "cache")]
    pub cache_read: f64,
    pub cache_write: f64,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Function {
    pub args: String,
    pub description: String,
    /// `false` keeps the function out of the agent's prompt and docs; code can still call it.
    pub show_agent: bool,
}

impl Default for Function {
    fn default() -> Self {
        Self {
            args: String::new(),
            description: String::new(),
            show_agent: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Hook {
    pub events: Vec<String>,
}

pub const HOOK_EVENTS: [&str; 8] = [
    "start",
    "close",
    "user_message",
    "agent_message",
    "model_change",
    "name_change",
    "lua_start",
    "lua_end",
];

pub const EFFORTS: [&str; 6] = ["default", "low", "medium", "high", "xhigh", "max"];

pub const BUILTIN_COMMANDS: [&str; 9] = [
    "config", "new", "resume", "models", "tasks", "session", "trace", "reload", "quit",
];

impl Config {
    /// The effective config as pretty JSON (without providers).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn connect(&self, name: &str) -> Option<&Connect> {
        self.connect.iter().find(|c| c.name == name)
    }

    /// Last definition wins when a user config overrides a plugin.
    pub fn model_def(&self, full: &str) -> Option<&ModelDef> {
        self.models.iter().rev().find(|m| m.name == full)
    }

    /// The model's `compact_at`, else `settings.compact_at`, else 60% of the model's window.
    pub fn compact_at(&self, def: Option<&ModelDef>) -> u64 {
        compact_at(def, self.settings.compact_at)
    }
}

/// Effective compaction threshold: model override, else `settings.compact_at`, else 60% of window.
pub fn compact_at(def: Option<&ModelDef>, setting: Option<u64>) -> u64 {
    def.and_then(|m| m.compact_at)
        .or(setting)
        .unwrap_or_else(|| def.map_or(0, |m| (u128::from(m.window) * 6 / 10) as u64))
}

pub fn display_name(def: Option<&ModelDef>, full: &str) -> String {
    match def {
        Some(m) if !m.display.is_empty() => m.display.clone(),
        _ => full.to_string(),
    }
}

/// Splits `provider/model` into its parts.
pub fn split_model(full: &str) -> (&str, &str) {
    full.split_once('/').unwrap_or(("", full))
}

/// `$VAR` or `${VAR}` as the whole value reads the environment; anything else is literal.
pub fn env_value(v: &str) -> String {
    let name = v
        .strip_prefix("${")
        .and_then(|r| r.strip_suffix('}'))
        .or_else(|| v.strip_prefix('$'));
    match name {
        Some(n) if !n.is_empty() && n.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()) => {
            std::env::var(n).unwrap_or_default()
        }
        _ => v.to_string(),
    }
}
