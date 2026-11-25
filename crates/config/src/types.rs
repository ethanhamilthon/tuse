use crate::names::{FunctionKind, HOST_FUNCTIONS};
use crate::theme::{Theme, UiSettings};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(skip)]
    pub connect: Vec<Connect>,
    pub settings: Settings,
    pub theme: Theme,
    #[serde(deserialize_with = "crate::de::list")]
    pub models: Vec<ModelDef>,
    #[serde(deserialize_with = "crate::de::list")]
    pub systems: Vec<String>,
    #[serde(deserialize_with = "crate::names::prompts")]
    pub prompts: BTreeMap<String, Prompt>,
    #[serde(deserialize_with = "crate::names::functions")]
    pub functions: BTreeMap<String, FunctionKind>,
    pub hooks: BTreeMap<String, Hook>,
    #[serde(deserialize_with = "crate::names::commands")]
    pub commands: Vec<String>,
    #[serde(skip_deserializing)]
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Connect {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub api: String,
}

impl Default for Connect {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_url: String::new(),
            api_key: String::new(),
            api: DEFAULT_API.to_string(),
        }
    }
}

impl Connect {
    pub fn api_type(&self) -> &str {
        if self.api.trim().is_empty() {
            DEFAULT_API
        } else {
            &self.api
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub ui: UiSettings,
    #[serde(deserialize_with = "crate::names::open")]
    pub open: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Prompt {
    pub kind: PromptKind,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptKind {
    #[default]
    Text,
    Function,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDef {
    #[serde(deserialize_with = "crate::names::model_name")]
    pub name: String,
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub window: u64,
    #[serde(default)]
    pub prices: Option<Prices>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Prices {
    #[serde(deserialize_with = "crate::de::price")]
    pub input: f64,
    #[serde(deserialize_with = "crate::de::price")]
    pub output: f64,
    #[serde(deserialize_with = "crate::de::price")]
    pub cache_read: f64,
    #[serde(deserialize_with = "crate::de::price")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hook {
    #[serde(deserialize_with = "crate::names::events")]
    pub events: Vec<String>,
}

pub const EFFORTS: [&str; 6] = ["default", "low", "medium", "high", "xhigh", "max"];

const DEFAULT_API: &str = "openai-chat";

impl Config {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn connect(&self, name: &str) -> Option<&Connect> {
        self.connect.iter().find(|c| c.name == name)
    }

    pub fn model_def(&self, full: &str) -> Option<&ModelDef> {
        self.models.iter().rev().find(|m| m.name == full)
    }

    pub fn is_disabled(&self, name: &str) -> bool {
        self.functions.get(name) == Some(&FunctionKind::Disabled)
    }

    pub fn disabled_builtins(&self) -> Vec<&'static str> {
        HOST_FUNCTIONS
            .iter()
            .copied()
            .filter(|n| self.is_disabled(n))
            .collect()
    }
}

pub fn display_name(def: Option<&ModelDef>, full: &str) -> String {
    match def {
        Some(m) if !m.display.is_empty() => m.display.clone(),
        _ => full.to_string(),
    }
}

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
