use crate::theme::Theme;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub model: String,
    pub provider: String,
    pub effort: String,
    pub connect: Vec<Connect>,
    pub settings: Settings,
    /// Legacy top-level `theme`; `settings.theme` wins.
    pub theme: String,
    pub themes: BTreeMap<String, Theme>,
    pub bins: BTreeMap<String, String>,
    pub models: Vec<ModelDef>,
    pub systems: Vec<String>,
    pub prompts: BTreeMap<String, String>,
    pub hooks: BTreeMap<String, Hook>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Connect {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub compact_at: u64,
    pub max_bash_output: usize,
    pub bash_timeout: u64,
    pub retries: usize,
    /// Name of the active theme; `default` or empty is the built-in one.
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            compact_at: 0,
            max_bash_output: 16384,
            bash_timeout: 120,
            retries: 5,
            theme: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ModelDef {
    pub name: String,
    pub display: String,
    pub window: u64,
    pub prices: Option<Prices>,
    pub compact_at: Option<u64>,
}

/// Prices in dollars per 1M tokens.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Prices {
    #[serde(rename = "in")]
    pub input: f64,
    pub out: f64,
    pub cache: f64,
    pub cache_write: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Hook {
    pub events: Vec<String>,
    pub run: String,
}

pub const EFFORTS: [&str; 6] = ["default", "low", "medium", "high", "xhigh", "max"];

impl Config {
    pub fn theme_name(&self) -> &str {
        match self.settings.theme.as_str() {
            "" => &self.theme,
            name => name,
        }
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

    /// The model-level threshold wins over `settings.compact_at`.
    pub fn compact_at(&self, full: &str) -> u64 {
        self.model_def(full)
            .and_then(|m| m.compact_at)
            .unwrap_or(self.settings.compact_at)
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
