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
    pub prompts: BTreeMap<String, String>,
    #[serde(deserialize_with = "crate::de::functions")]
    pub functions: BTreeMap<String, FunctionKind>,
    pub hooks: BTreeMap<String, Hook>,
    #[serde(deserialize_with = "crate::de::commands")]
    pub commands: Vec<String>,
    #[serde(deserialize_with = "crate::de::open")]
    pub open: Vec<String>,
    #[serde(skip_deserializing)]
    pub path: String,
    #[serde(skip_deserializing)]
    pub plugins: Vec<PluginInfo>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub dir: String,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
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
        if self.api.is_empty() {
            DEFAULT_API
        } else {
            &self.api
        }
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::de::opt_u64"
    )]
    pub compact_at: Option<u64>,
    #[serde(deserialize_with = "crate::de::int::<_, _, 1, { i64::MAX }>")]
    pub max_image_bytes: usize,
    #[serde(deserialize_with = "crate::de::int::<_, _, 0, 100>")]
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDef {
    #[serde(deserialize_with = "crate::de::model_name")]
    pub name: String,
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub window: u64,
    #[serde(default)]
    pub prices: Option<Prices>,
    #[serde(default)]
    pub compact_at: Option<u64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FunctionKind {
    Custom,
    Override,
    Disabled,
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hook {
    #[serde(deserialize_with = "crate::de::events")]
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

pub const DEFAULT_API: &str = "openai-chat";

pub const BUILTIN_COMMANDS: [&str; 9] = [
    "config", "new", "resume", "models", "tasks", "session", "trace", "reload", "quit",
];

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

    pub fn compact_at(&self, def: Option<&ModelDef>) -> u64 {
        compact_at(def, self.settings.compact_at)
    }

    pub fn is_disabled(&self, name: &str) -> bool {
        self.functions.get(name) == Some(&FunctionKind::Disabled)
    }

    pub fn disabled_builtins(&self) -> Vec<&'static str> {
        crate::HOST_FUNCTIONS
            .iter()
            .copied()
            .filter(|n| self.is_disabled(n))
            .collect()
    }
}

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

pub fn split_model(full: &str) -> (&str, &str) {
    full.split_once('/').unwrap_or(("", full))
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
