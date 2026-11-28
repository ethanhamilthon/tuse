use crate::names::{FunctionKind, HOST_FUNCTIONS};
use crate::ui::Ui;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum Systems {
    Replace(String),
    Named(BTreeMap<String, String>),
}

impl Default for Systems {
    fn default() -> Self {
        Self::Named(BTreeMap::new())
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub ui: Ui,
    pub open: bool,
    #[serde(deserialize_with = "crate::names::model_map")]
    pub models: BTreeMap<String, ModelDef>,
    pub systems: Systems,
    pub prompts: BTreeMap<String, String>,
    pub functions: BTreeMap<String, FunctionKind>,
    #[serde(deserialize_with = "crate::names::hook")]
    pub hook: BTreeMap<String, Vec<String>>,
    pub commands: std::collections::BTreeSet<String>,
    #[serde(skip_deserializing)]
    pub plugins: Vec<crate::PluginSummary>,
    #[serde(skip_deserializing)]
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Connect {
    pub name: String,
    pub base_url: String,
    #[serde(skip_serializing)]
    pub api_key: String,
    pub api: String,
    #[serde(skip_serializing)]
    pub headers: BTreeMap<String, String>,
}

impl Default for Connect {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_url: String::new(),
            api_key: String::new(),
            api: DEFAULT_API.to_string(),
            headers: BTreeMap::new(),
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

pub fn command_words(text: &str) -> Result<Vec<String>, &'static str> {
    let mut words = vec![];
    let mut word = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(q) if c == '\\' && chars.peek() == Some(&q) => {
                word.push(q);
                chars.next();
            }
            Some(_) => word.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            None => {
                word.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err("unclosed quote");
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDef {
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub window: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::names::model_efforts"
    )]
    pub efforts: Option<Vec<String>>,
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

pub const DEFAULT_EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

const DEFAULT_API: &str = "openai-chat";

impl Config {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn model_def(&self, full: &str) -> Option<&ModelDef> {
        self.models.get(full)
    }

    pub fn model_efforts(&self, full: &str) -> Vec<String> {
        match self.model_def(full).and_then(|m| m.efforts.as_ref()) {
            Some(list) => list.clone(),
            None => DEFAULT_EFFORTS.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    pub fn fallback_effort(&self, full: &str) -> String {
        let list = self.model_efforts(full);
        if list.is_empty() {
            String::new()
        } else if list.iter().any(|e| e == "medium") {
            "medium".into()
        } else {
            list[0].clone()
        }
    }

    pub fn check_effort(&self, full: &str, effort: &str) -> Result<(), String> {
        let list = self.model_efforts(full);
        if list.is_empty() {
            if effort.is_empty() {
                return Ok(());
            }
            return Err(format!("model `{full}` does not support reasoning efforts"));
        }
        if list.iter().any(|e| e == effort) {
            Ok(())
        } else {
            Err(format!(
                "effort `{effort}` is not supported by `{full}` (supported: {})",
                list.join(", ")
            ))
        }
    }

    pub fn is_disabled(&self, name: &str) -> bool {
        if self.functions.get(name) == Some(&FunctionKind::Disabled) {
            return true;
        }
        let mut cur = name;
        while let Some((prefix, _)) = cur.rsplit_once('.') {
            if self.functions.get(prefix) == Some(&FunctionKind::Disabled) {
                return true;
            }
            cur = prefix;
        }
        false
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

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        command_words(text).unwrap()
    }

    #[test]
    fn words_split_on_whitespace_and_honor_quotes() {
        assert_eq!(words("  a  b "), ["a", "b"]);
        assert_eq!(words(r#"fix "the bug" 'x y'"#), ["fix", "the bug", "x y"]);
        assert_eq!(words(r#""say \"hi\"" """#), [r#"say "hi""#, ""]);
        assert!(words("").is_empty());
        assert_eq!(command_words(r#"a "b"#), Err("unclosed quote"));
    }
}
