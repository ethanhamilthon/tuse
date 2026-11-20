use crate::theme::{Theme, UiSettings};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Config {
    pub model: String,
    pub provider: String,
    pub effort: String,
    pub connect: Vec<Connect>,
    pub settings: Settings,
    pub themes: BTreeMap<String, Theme>,
    pub models: Vec<ModelDef>,
    pub systems: Vec<String>,
    pub prompts: BTreeMap<String, String>,
    pub functions: BTreeMap<String, Function>,
    pub hooks: BTreeMap<String, Hook>,
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Connect {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Settings {
    /// Unset: 60% of the model's `window` when it is known; `0` turns compaction off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_at: Option<u64>,
    pub max_output: usize,
    pub tool_timeout: u64,
    pub hook_timeout: u64,
    pub max_image_bytes: usize,
    pub retries: usize,
    /// Name of the active theme; `default` or empty is the built-in one.
    pub theme: String,
    pub ui: UiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            compact_at: None,
            max_output: 4096,
            tool_timeout: 10,
            hook_timeout: 30,
            max_image_bytes: 5 * 1024 * 1024,
            retries: 5,
            theme: String::new(),
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

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Function {
    pub args: String,
    pub description: String,
    pub run: String,
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Hook {
    pub events: Vec<String>,
    /// Python code, run in the hook's persistent interpreter.
    pub run: String,
}

pub const HOOK_EVENTS: [&str; 4] = [
    "process_start",
    "process_end",
    "user_message",
    "agent_message",
];

pub const EFFORTS: [&str; 6] = ["default", "low", "medium", "high", "xhigh", "max"];

impl Config {
    pub fn to_yaml(&self) -> Result<String, String> {
        let mut effective = self.clone();
        effective.themes.entry("default".into()).or_default();
        serde_yaml::to_string(&effective).map_err(|e| e.to_string())
    }

    pub fn theme_name(&self) -> &str {
        &self.settings.theme
    }

    /// The active theme.
    pub fn active_theme(&self) -> Theme {
        self.themes
            .get(self.theme_name())
            .cloned()
            .unwrap_or_default()
    }

    /// The full `provider/model` name of the configured default model.
    pub fn default_model(&self) -> String {
        full_model_name(&self.model, &self.provider, &self.connect)
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

    pub fn connect(&self, name: &str) -> Option<&Connect> {
        self.connect.iter().find(|c| c.name == name)
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

pub fn full_model_name(model: &str, provider: &str, connect: &[Connect]) -> String {
    if let Some((p, _)) = model.split_once('/')
        && connect.iter().any(|c| c.name == p)
    {
        return model.to_string();
    }
    let provider = if provider.is_empty() {
        connect.first().map(|c| c.name.as_str()).unwrap_or("")
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
