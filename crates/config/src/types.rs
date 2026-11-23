use crate::mode::{Mode, ModeEntry};
use crate::theme::{Theme, UiSettings};
use serde::Deserialize;

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Config {
    pub model: String,
    pub provider: String,
    pub effort: String,
    /// Providers come from the database (`tuse connect`), never from the file.
    #[serde(skip)]
    pub connect: Vec<Connect>,
    pub settings: Settings,
    /// The 10 colors of the UI.
    pub theme: Theme,
    pub models: Vec<ModelDef>,
    pub systems: Vec<ModeEntry>,
    pub prompts: BTreeMap<String, ModeEntry>,
    pub functions: BTreeMap<String, Function>,
    pub hooks: BTreeMap<String, Hook>,
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Connect {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<Mode>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
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

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ModelDef {
    pub name: String,
    pub display: String,
    pub window: u64,
    pub prices: Option<Prices>,
    pub compact_at: Option<u64>,
}

/// Prices in dollars per 1M tokens.
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Prices {
    #[serde(rename = "in")]
    pub input: f64,
    pub out: f64,
    pub cache: f64,
    pub cache_write: f64,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Function {
    pub args: String,
    pub description: String,
    /// `false` keeps the function out of the agent's prompt and docs; code can still call it.
    pub show_agent: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<Mode>,
}

impl Default for Function {
    fn default() -> Self {
        Self {
            args: String::new(),
            description: String::new(),
            show_agent: true,
            modes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Hook {
    pub events: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<Mode>,
}

pub const HOOK_EVENTS: [&str; 4] = [
    "process_start",
    "process_end",
    "user_message",
    "agent_message",
];

pub const EFFORTS: [&str; 6] = ["default", "low", "medium", "high", "xhigh", "max"];

impl Config {
    /// The effective config as pretty JSON (without providers).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// The full `provider/model` name of the configured default model.
    pub fn default_model(&self, mode: Mode) -> String {
        full_model_name(&self.model, &self.provider, &self.connect, mode)
    }

    /// What is wrong in the config but not fatal: each item is one notice.
    pub fn problems(&self) -> Vec<String> {
        let mut out = vec![];
        if !self.effort.is_empty() && !EFFORTS.contains(&self.effort.as_str()) {
            out.push(format!(
                "config: unknown effort `{}`, using `default`; use one of {}",
                self.effort,
                EFFORTS.join(", ")
            ));
        }
        for (name, hook) in &self.hooks {
            for e in hook
                .events
                .iter()
                .filter(|e| !HOOK_EVENTS.contains(&e.as_str()))
            {
                out.push(format!(
                    "hook {name}: unknown event `{e}`; events are {}",
                    HOOK_EVENTS.join(", ")
                ));
            }
        }
        out
    }

    pub fn default_effort(&self) -> String {
        if EFFORTS.contains(&self.effort.as_str()) {
            self.effort.clone()
        } else {
            "default".into()
        }
    }

    pub fn model_def(&self, full: &str) -> Option<&ModelDef> {
        self.models.iter().find(|m| m.name == full)
    }

    pub fn connect(&self, name: &str, mode: Mode) -> Option<&Connect> {
        self.connect
            .iter()
            .find(|c| c.matches(mode) && c.name == name)
    }

    /// The model's `compact_at`, else `settings.compact_at`, else 60% of the model's window.
    pub fn compact_at(&self, full: &str) -> u64 {
        let def = self.model_def(full);
        def.and_then(|m| m.compact_at)
            .or(self.settings.compact_at)
            .unwrap_or_else(|| def.map_or(0, |m| m.window * 6 / 10))
    }

    pub fn display_name(&self, full: &str) -> String {
        match self.model_def(full) {
            Some(m) if !m.display.is_empty() => m.display.clone(),
            _ => full.to_string(),
        }
    }
}

pub fn full_model_name(model: &str, provider: &str, connect: &[Connect], mode: Mode) -> String {
    if let Some((p, _)) = model.split_once('/')
        && connect.iter().any(|c| c.name == p)
    {
        return model.to_string();
    }
    let provider = if provider.is_empty() {
        connect
            .iter()
            .find(|c| c.matches(mode))
            .map(|c| c.name.as_str())
            .unwrap_or("")
    } else {
        provider
    };
    if provider.is_empty() {
        model.to_string()
    } else {
        format!("{provider}/{model}")
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
