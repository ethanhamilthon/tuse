use crate::de::list;
use serde::Deserialize;
use serde::de::{Deserializer, Error};
use std::collections::BTreeMap;

pub const HOST_FUNCTIONS: &[&str] = &[
    "sh",
    "spawn",
    "read",
    "write",
    "append",
    "delete",
    "edit",
    "grep",
    "docs",
    "set_session",
    "session",
    "fork",
    "resume",
    "task_get",
    "task_list",
    "task_stop",
    "show",
    "pick",
    "ask",
    "reload",
    "llm",
    "json",
    "inspect",
];

const RESERVED: &str = "and break do else elseif end false for function goto if in local nil not or repeat return then true until while _G _VERSION assert collectgarbage dofile error getmetatable ipairs load loadfile next pairs pcall print rawequal rawget rawlen rawset require select setmetatable tonumber tostring type warn xpcall coroutine io math os package string table utf8";

pub fn check_function_name(name: &str) -> Option<&'static str> {
    let mut chars = name.chars();
    if !chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        || !chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
    {
        Some("invalid Lua name")
    } else if RESERVED.split_whitespace().any(|r| r == name) {
        Some("reserved name")
    } else {
        None
    }
}

pub fn check_command_name(name: &str) -> Option<&'static str> {
    if name.is_empty()
        || name.starts_with('/')
        || name.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        Some("invalid command name")
    } else if BUILTIN_COMMANDS.contains(&name) {
        Some("cannot override built-in command")
    } else {
        None
    }
}

pub const OPEN_DIR: &str = "@dir";
pub const OPEN_URL: &str = "@url";

fn is_mask(key: &str) -> bool {
    key.contains(['*', '?'])
}

pub fn check_open_ext(key: &str) -> Option<&'static str> {
    if key.is_empty() {
        Some("extension must not be empty")
    } else if key
        .chars()
        .any(|c| c == '/' || c == '\\' || c.is_whitespace() || c.is_control())
    {
        Some("invalid file extension")
    } else if key == OPEN_DIR || key == OPEN_URL || is_mask(key) {
        None
    } else if key.starts_with('@') {
        Some("unknown special key; use `@dir` or `@url`")
    } else if key.starts_with('.') {
        Some("extension must not start with `.`")
    } else {
        None
    }
}

pub fn open_label(key: &str) -> String {
    if key.starts_with('@') || is_mask(key) {
        key.to_string()
    } else {
        format!(".{key}")
    }
}

fn glob(p: &[char], s: &[char]) -> bool {
    match p.split_first() {
        None => s.is_empty(),
        Some(('*', rest)) => (0..=s.len()).any(|i| glob(rest, &s[i..])),
        Some(('?', rest)) => !s.is_empty() && glob(rest, &s[1..]),
        Some((c, rest)) => s.first() == Some(c) && glob(rest, &s[1..]),
    }
}

fn literals(key: &str) -> usize {
    key.chars().filter(|c| !matches!(c, '*' | '?')).count()
}

pub fn pick_open_key<'a>(
    keys: &'a [String],
    path: &std::path::Path,
    dir: bool,
) -> Option<&'a String> {
    if dir {
        let find = |k: &str| keys.iter().find(|x| x.as_str() == k);
        return find(OPEN_DIR).or_else(|| find("*"));
    }
    let ext = path.extension().and_then(|e| e.to_str());
    if let Some(ext) = ext
        && let Some(k) = keys
            .iter()
            .find(|k| !k.starts_with('@') && !is_mask(k) && k.eq_ignore_ascii_case(ext))
    {
        return Some(k);
    }
    let name: Vec<char> = path.file_name()?.to_str()?.to_lowercase().chars().collect();
    keys.iter()
        .filter(|k| is_mask(k) && glob(&k.to_lowercase().chars().collect::<Vec<_>>(), &name))
        .max_by(|a, b| literals(a).cmp(&literals(b)).then_with(|| b.cmp(a)))
}

pub const HOOK_EVENTS: [&str; 13] = [
    "config_load",
    "process_start",
    "process_end",
    "session_created",
    "session_resumed",
    "session_close",
    "session_updated",
    "user_message",
    "agent_message",
    "turn_error",
    "turn_stopped",
    "lua_start",
    "lua_end",
];

pub const BUILTIN_COMMANDS: [&str; 11] = [
    "config", "new", "resume", "fork", "models", "tasks", "session", "history", "reload", "stop",
    "quit",
];

pub const HISTORY_ROLES: [&str; 15] = [
    "system",
    "user",
    "agent",
    "lua",
    "notification",
    "message",
    "usage",
    "meta",
    "turn_end",
    "show",
    "pick",
    "ask",
    "task",
    "command",
    "open",
];

pub const MESSAGE_LEVELS: [&str; 3] = ["flash", "notice", "error"];

pub fn history_roles<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list: Vec<String> = list(d)?;
    if let Some((i, r)) = list.iter().enumerate().find(|(_, r)| {
        !HISTORY_ROLES.contains(&r.as_str()) && !MESSAGE_LEVELS.contains(&r.as_str())
    }) {
        return Err(D::Error::custom(format!(
            "history.hide[{}]: unknown history role `{r}`",
            i + 1
        )));
    }
    Ok(list)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PromptValue {
    Text(String),
    Full(crate::types::Prompt),
}

pub fn prompts<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, crate::types::Prompt>, D::Error> {
    let map = BTreeMap::<String, PromptValue>::deserialize(d)?;
    let mut out = BTreeMap::new();
    for (name, v) in map {
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return Err(D::Error::custom(format!("`{name}`: invalid prompt name")));
        }
        let p = match v {
            PromptValue::Text(body) => crate::types::Prompt {
                body,
                ..Default::default()
            },
            PromptValue::Full(p) => p,
        };
        out.insert(name, p);
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FunctionKind {
    Custom,
    Override,
    Disabled,
}

pub fn split_model(full: &str) -> (&str, &str) {
    full.split_once('/').unwrap_or(("", full))
}

pub fn model_name<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = String::deserialize(d)?;
    let (provider, id) = split_model(&s);
    if provider.is_empty() || id.is_empty() || s.chars().any(char::is_whitespace) {
        return Err(D::Error::custom(format!(
            "expected a provider/model name, got `{s}`"
        )));
    }
    Ok(s)
}

pub fn events<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list: Vec<String> = list(d)?;
    if list.is_empty() {
        return Err(D::Error::custom("must contain at least one event"));
    }
    if let Some(e) = list.iter().find(|e| !HOOK_EVENTS.contains(&e.as_str())) {
        return Err(D::Error::custom(format!(
            "unknown event `{e}`; events are {}",
            HOOK_EVENTS.join(", ")
        )));
    }
    Ok(list)
}

fn names<E: Error>(list: &[String], check: fn(&str) -> Option<&'static str>) -> Result<(), E> {
    match list.iter().find_map(|n| check(n).map(|r| (n, r))) {
        Some((n, reason)) => Err(E::custom(format!("`{n}`: {reason}"))),
        None => Ok(()),
    }
}

pub fn functions<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, FunctionKind>, D::Error> {
    let map = BTreeMap::<String, FunctionKind>::deserialize(d)?;
    for (name, kind) in &map {
        if let Some(reason) = check_function_name(name) {
            return Err(D::Error::custom(format!("'{name}': {reason}")));
        }
        if name == "json" && *kind != FunctionKind::Disabled {
            return Err(D::Error::custom(
                "'json' is a table and can only be disabled with false",
            ));
        }
    }
    Ok(map)
}

pub fn commands<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, crate::types::Command>, D::Error> {
    let map = BTreeMap::<String, crate::types::Command>::deserialize(d)?;
    if let Some((name, reason)) = map
        .keys()
        .find_map(|n| check_command_name(n).map(|r| (n, r)))
    {
        return Err(D::Error::custom(format!("`{name}`: {reason}")));
    }
    Ok(map)
}

pub fn open<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list: Vec<String> = list(d)?;
    names(&list, check_open_ext)?;
    Ok(list)
}

#[cfg(test)]
mod open_tests {
    use super::*;
    use std::path::Path;

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn open_keys_are_checked_and_matched() {
        assert!(check_open_ext("@file").is_some());
        assert!(check_open_ext(".md").is_some());
        assert!(check_open_ext("a b").is_some());
        for ok in ["md", "*", "@dir", "@url", ".env*", "*.test.lua"] {
            assert!(check_open_ext(ok).is_none(), "{ok}");
        }
        let k = keys(&["lua", "*.test.lua", "*", "@dir", "t*"]);
        let pick = |p: &str, dir| pick_open_key(&k, Path::new(p), dir).map(String::as_str);
        assert_eq!(pick("/a/x.LUA", false), Some("lua"));
        assert_eq!(pick("/a/x.test.lua", false), Some("lua"));
        assert_eq!(pick("/a/x.txt", false), Some("*"));
        assert_eq!(pick("/a/T.txt", false), Some("t*"));
        assert_eq!(pick("/a/b", true), Some("@dir"));
        let k = keys(&["*.test.lua", "*.lua", "*"]);
        assert_eq!(
            pick_open_key(&k, Path::new("a.TEST.lua"), false).map(String::as_str),
            Some("*.test.lua")
        );
        assert_eq!(
            pick_open_key(&k, Path::new("a"), true).map(String::as_str),
            Some("*")
        );
        assert_eq!(open_label("md"), ".md");
        assert_eq!(open_label("*.md"), "*.md");
    }
}
