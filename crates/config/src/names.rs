use serde::Deserialize;
use serde::de::{Deserializer, Error};
use std::collections::BTreeMap;

pub const HOST_GROUPS: &[&str] = &["fs", "tasks", "session", "ui", "plugins", "providers"];

pub const HOST_FUNCTIONS: &[&str] = &[
    "sh",
    "reload",
    "llm",
    "fs.read",
    "fs.write",
    "fs.append",
    "fs.delete",
    "fs.edit",
    "fs.grep",
    "tasks.get",
    "tasks.spawn",
    "tasks.list",
    "tasks.stop",
    "session.get",
    "session.set",
    "session.fork",
    "session.resume",
    "ui.show",
    "ui.pick",
    "ui.ask",
    "plugins.list",
    "plugins.get",
    "plugins.add",
    "plugins.delete",
    "plugins.update",
    "providers.connect",
    "providers.disconnect",
    "providers.list",
    "providers.update",
    "docs",
    "json",
];

const RESERVED: &str = "and break do else elseif end false for function goto if in local nil not or repeat return then true until while _G _VERSION assert collectgarbage dofile error getmetatable ipairs load loadfile next pairs pcall print rawequal rawget rawlen rawset require select setmetatable tonumber tostring type warn xpcall coroutine io math os package string table utf8";

pub fn check_function_name(name: &str) -> Option<&'static str> {
    let mut chars = name.chars();
    let ok = chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric());
    if !ok {
        Some("invalid Lua name")
    } else if RESERVED.split_whitespace().any(|r| r == name) {
        Some("reserved name")
    } else {
        None
    }
}

pub fn check_function_path(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        return Some("invalid Lua name");
    }
    for seg in path.split('.') {
        if let Some(reason) = check_function_name(seg) {
            return Some(reason);
        }
    }
    None
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
    "task_message",
    "ui_prompt",
];

pub const BUILTIN_COMMANDS: [&str; 13] = [
    "config",
    "new",
    "resume",
    "fork",
    "models",
    "providers",
    "plugins",
    "tasks",
    "session",
    "history",
    "reload",
    "stop",
    "quit",
];

pub const HISTORY_ROLES: [&str; 16] = [
    "system",
    "user",
    "agent",
    "lua",
    "notification",
    "task_message",
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

pub fn model_efforts<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<String>>, D::Error> {
    struct Seed<'a>(&'a mut Vec<String>);
    impl<'de> serde::de::DeserializeSeed<'de> for Seed<'_> {
        type Value = ();
        fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
            let s = String::deserialize(d)?;
            if !crate::DEFAULT_EFFORTS.contains(&s.as_str()) {
                return Err(D::Error::custom(format!(
                    "unknown effort `{s}` (supported: {})",
                    crate::DEFAULT_EFFORTS.join(", ")
                )));
            }
            if self.0.contains(&s) {
                return Err(D::Error::custom(format!("duplicate effort `{s}`")));
            }
            self.0.push(s);
            Ok(())
        }
    }
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = Option<Vec<String>>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a list of effort names")
        }
        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_any(V)
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = vec![];
            while seq.next_element_seed(Seed(&mut out))?.is_some() {}
            Ok(Some(out))
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            match map.next_key::<String>()? {
                None => Ok(Some(vec![])),
                Some(k) => Err(A::Error::custom(format!("expected a list, got key `{k}`"))),
            }
        }
    }
    d.deserialize_any(V)
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

pub fn model_map<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, crate::ModelDef>, D::Error> {
    let map = BTreeMap::<String, serde_json::Value>::deserialize(d)?;
    map.into_iter()
        .map(|(name, body)| {
            let (provider, id) = split_model(&name);
            if provider.is_empty() || id.is_empty() || name.chars().any(char::is_whitespace) {
                return Err(D::Error::custom(format!(
                    "expected a provider/model name, got `{name}`"
                )));
            }
            let model = serde_path_to_error::deserialize(body)
                .map_err(|e| D::Error::custom(format!("{name}: {e}")))?;
            Ok((name, model))
        })
        .collect()
}

pub fn hook<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, Vec<String>>, D::Error> {
    let map = BTreeMap::<String, Vec<String>>::deserialize(d)?;
    if let Some(e) = map.keys().find(|e| !HOOK_EVENTS.contains(&e.as_str())) {
        return Err(D::Error::custom(format!(
            "unknown event `{e}`; events are {}",
            HOOK_EVENTS.join(", ")
        )));
    }
    Ok(map)
}
