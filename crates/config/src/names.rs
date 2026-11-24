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
    "set_title",
    "session",
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

pub fn check_open_ext(ext: &str) -> Option<&'static str> {
    if ext.is_empty() {
        Some("extension must not be empty")
    } else if ext.starts_with('.') {
        Some("extension must not start with `.`")
    } else if ext
        .chars()
        .any(|c| c == '/' || c == '\\' || c.is_whitespace() || c.is_control())
    {
        Some("invalid file extension")
    } else {
        None
    }
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

pub const BUILTIN_COMMANDS: [&str; 9] = [
    "config", "new", "resume", "models", "tasks", "session", "trace", "reload", "quit",
];

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

pub fn commands<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list: Vec<String> = list(d)?;
    names(&list, check_command_name)?;
    Ok(list)
}

pub fn open<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list: Vec<String> = list(d)?;
    names(&list, check_open_ext)?;
    Ok(list)
}
