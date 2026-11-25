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
    pub commands: BTreeMap<String, Command>,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Command {
    pub desc: String,
    pub args: Vec<String>,
    pub params: Option<usize>,
}

impl Command {
    pub fn check(&self, name: &str, words: &[String]) -> Result<(), String> {
        if !self.args.is_empty() {
            return match words {
                [] => Ok(()),
                [w] if self.args.contains(w) => Ok(()),
                [w] => Err(format!("Unknown arg {w} for /{name}")),
                _ => Err(format!("/{name} takes at most 1 word")),
            };
        }
        match self.params {
            Some(0) if !words.is_empty() => Err(format!("/{name} takes no words")),
            Some(n) if words.len() > n => Err(format!(
                "/{name} takes at most {n} word{}",
                if n == 1 { "" } else { "s" }
            )),
            _ => Ok(()),
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

    #[test]
    fn commands_accept_words_by_args_or_parameter_count() {
        let w = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let free = Command {
            params: Some(2),
            ..Default::default()
        };
        assert!(free.check("mv", &w(&["a", "b"])).is_ok());
        assert_eq!(
            free.check("mv", &w(&["a", "b", "c"])),
            Err("/mv takes at most 2 words".into())
        );
        let none = Command {
            params: Some(0),
            ..Default::default()
        };
        assert_eq!(
            none.check("diff", &w(&["x"])),
            Err("/diff takes no words".into())
        );
        let variadic = Command::default();
        assert!(variadic.check("run", &w(&["a", "b", "c"])).is_ok());
        let listed = Command {
            args: w(&["staging", "prod"]),
            params: Some(1),
            ..Default::default()
        };
        assert!(listed.check("deploy", &w(&["prod"])).is_ok());
        assert!(listed.check("deploy", &[]).is_ok());
        assert_eq!(
            listed.check("deploy", &w(&["nope"])),
            Err("Unknown arg nope for /deploy".into())
        );
        assert!(listed.check("deploy", &w(&["staging", "prod"])).is_err());
    }
}
