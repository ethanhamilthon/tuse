//! Config checks: serde validates the data (see `tuse_config::from_value`); this file adds only
//! what serde cannot see in JSON: which values must be Lua functions.

use crate::host::to_json;
use tuse_lua::mlua::{Lua, Table, Value};

/// Validates one config table (the user's or a plugin's) and returns it as a `Config`.
pub(crate) fn table(
    lua: &Lua,
    t: &Table,
    source: &str,
    is_plugin: bool,
) -> Result<tuse_config::Config, String> {
    let fail = |path: String, msg: &str| Err(format!("{source}: {path}: {msg}"));
    if is_plugin && !t.raw_get::<Value>("plugins").is_ok_and(|v| v.is_nil()) {
        return fail("plugins".into(), "is not allowed in a plugin");
    }
    for key in ["functions", "hooks"] {
        for (name, v) in entries(t, key) {
            let Value::Table(entry) = v else { continue };
            match entry.raw_get::<Value>("run") {
                Ok(Value::Function(_)) => {}
                Ok(Value::Nil) => return fail(format!("{key}.{name}.run"), "is required"),
                Ok(other) => return fail(format!("{key}.{name}.run"), &expected(&other)),
                Err(e) => return fail(format!("{key}.{name}.run"), &e.to_string()),
            }
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

/// The config table as JSON: Lua functions dropped, `commands` and `open` as their names.
pub(crate) fn json(lua: &Lua, t: &Table) -> serde_json::Value {
    let mut raw = to_json(lua, Value::Table(t.clone()));
    if let Some(obj) = raw.as_object_mut() {
        obj.remove("plugins");
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
