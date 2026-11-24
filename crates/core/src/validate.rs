use crate::host::to_json;
use std::collections::HashSet;
use tuse_lua::mlua::{Lua, Table, Value};

pub(crate) fn table(
    lua: &Lua,
    t: &Table,
    source: &str,
    is_plugin: bool,
    allow_deferred_false: bool,
) -> Result<tuse_config::Config, String> {
    let fail = |path: String, msg: &str| Err(format!("{source}: {path}: {msg}"));
    if is_plugin && !t.raw_get::<Value>("plugins").is_ok_and(|v| v.is_nil()) {
        return fail("plugins".into(), "is not allowed in a plugin");
    }
    validate_functions(t, source, is_plugin, allow_deferred_false)?;
    for (name, v) in entries(t, "hooks") {
        let Value::Table(entry) = v else { continue };
        match entry.raw_get::<Value>("run") {
            Ok(Value::Function(_)) => {}
            Ok(Value::Nil) => return fail(format!("hooks.{name}.run"), "is required"),
            Ok(other) => return fail(format!("hooks.{name}.run"), &expected(&other)),
            Err(e) => return fail(format!("hooks.{name}.run"), &e.to_string()),
        }
    }
    for key in ["commands", "open"] {
        for (name, v) in entries(t, key) {
            if !matches!(v, Value::Function(_)) {
                return fail(format!("{key}.{name}"), &expected(&v));
            }
        }
    }
    tuse_config::from_value(json(lua, t), source)
}

fn validate_functions(
    t: &Table,
    source: &str,
    is_plugin: bool,
    allow_deferred_false: bool,
) -> Result<(), String> {
    let Ok(raw) = t.raw_get::<Value>("functions") else {
        return Ok(());
    };
    let Value::Table(_) = raw else {
        return Ok(());
    };
    let mut list = entries(t, "functions");
    list.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, v) in list {
        if let Some(reason) = tuse_config::check_function_name(&name) {
            return Err(format!("{source}: functions: '{name}': {reason}"));
        }
        match v {
            Value::Function(_) => {
                if name == "json" {
                    return Err(format!(
                        "{source}: functions.json: 'json' is a table and can only be disabled with false"
                    ));
                }
            }
            Value::Boolean(false) => {
                if !tuse_config::HOST_FUNCTIONS.contains(&name.as_str())
                    && (is_plugin || !allow_deferred_false)
                {
                    return Err(format!(
                        "{source}: functions.{name}: cannot disable unknown function"
                    ));
                }
            }
            Value::Table(_) => {
                return Err(format!(
                    "{source}: functions.{name}: expected function or false, got table (the {{ run, description, args, show_agent }} form was removed; pass a function directly or false to disable)"
                ));
            }
            other => {
                return Err(format!(
                    "{source}: functions.{name}: expected function or false, got {}",
                    other.type_name()
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn check_disabled_targets(
    t: &Table,
    source: &str,
    plugin_fns: &HashSet<String>,
) -> Result<(), String> {
    let mut list = entries(t, "functions");
    list.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, v) in list {
        if matches!(v, Value::Boolean(false))
            && !tuse_config::HOST_FUNCTIONS.contains(&name.as_str())
            && !plugin_fns.contains(&name)
        {
            return Err(format!(
                "{source}: functions.{name}: cannot disable unknown function"
            ));
        }
    }
    Ok(())
}

pub(crate) fn function_names(t: &Table) -> Vec<String> {
    entries(t, "functions")
        .into_iter()
        .filter_map(|(n, v)| matches!(v, Value::Function(_)).then_some(n))
        .collect()
}

fn expected(v: &Value) -> String {
    format!("expected function, got {}", v.type_name())
}

fn entries(t: &Table, key: &str) -> Vec<(String, Value)> {
    let Ok(Value::Table(sub)) = t.raw_get::<Value>(key) else {
        return vec![];
    };
    sub.pairs::<Value, Value>()
        .flatten()
        .map(|(k, v)| (key_str(&k), v))
        .collect()
}

pub(crate) fn json(lua: &Lua, t: &Table) -> serde_json::Value {
    let mut raw = to_json(lua, Value::Table(t.clone()));
    if let Some(obj) = raw.as_object_mut() {
        obj.remove("plugins");
        if t.raw_get::<Value>("functions").is_ok_and(|v| v.is_table()) {
            let mut map = serde_json::Map::new();
            for (name, v) in entries(t, "functions") {
                let kind = if matches!(v, Value::Boolean(false)) {
                    "disabled"
                } else if tuse_config::HOST_FUNCTIONS.contains(&name.as_str()) {
                    "override"
                } else {
                    "custom"
                };
                map.insert(name, serde_json::Value::String(kind.into()));
            }
            obj.insert("functions".into(), serde_json::Value::Object(map));
        }
        for key in ["commands", "open"] {
            if t.raw_get::<Value>(key).is_ok_and(|v| v.is_table()) {
                let mut names: Vec<String> = entries(t, key).into_iter().map(|e| e.0).collect();
                names.sort();
                obj.insert(key.into(), serde_json::json!(names));
            }
        }
    }
    raw
}

pub(crate) fn key_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string_lossy(),
        Value::Integer(n) => n.to_string(),
        Value::Number(n) => n.to_string(),
        other => format!("<{}>", other.type_name()),
    }
}

pub(crate) fn kind(v: &Value) -> &'static str {
    v.type_name()
}
