use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;
use tuse_config::{Config, HOOK_EVENTS, HOST_FUNCTIONS, HOST_GROUPS, check_function_name};
use tuse_lua::mlua::{Lua, Table, Value};

pub(crate) type Namespaces = BTreeMap<String, (String, Table)>;

fn table(v: Value, path: &str) -> Result<Table, String> {
    match v {
        Value::Table(t) => Ok(t),
        _ => Err(format!("{path}: expected a table")),
    }
}

pub(crate) fn insert(
    owners: &mut Namespaces,
    name: String,
    src: &str,
    value: Value,
    plugin: bool,
) -> Result<(), String> {
    let valid = if plugin {
        tuse_config::validate_plugin_name(&name).is_ok()
            && !name.contains('.')
            && check_function_name(&name) != Some("reserved name")
    } else {
        check_function_name(&name).is_none()
    };
    if !valid || HOST_GROUPS.contains(&name.as_str()) || HOST_FUNCTIONS.contains(&name.as_str()) {
        return Err(format!("{src}: {name}: invalid or reserved namespace name"));
    }
    if let Some((old, _)) = owners.get(&name) {
        return Err(format!(
            "{src}: duplicate namespace `{name}` (already declared by {old})"
        ));
    }
    let t = table(value, &format!("{src}: {name}"))?;
    owners.insert(name, (src.into(), t));
    Ok(())
}

pub(crate) fn global(
    value: Value,
    src: &str,
    owners: &mut Namespaces,
) -> Result<(Config, Option<Value>), String> {
    let mut settings = Map::new();
    let mut open = None;
    for (key, value) in str_entries(&table(value, src)?, src, None)? {
        let path = format!("{src}: {key}");
        match key.as_str() {
            "ui" | "systems" | "models" => {
                settings.insert(key, to_json(&value, &path, 0)?);
            }
            "open" if matches!(value, Value::Function(_)) => open = Some(value),
            "open" => return Err(format!("{path}: expected a function")),
            "global" | "history" | "plugins" => {
                return Err(format!("{path}: configuration key was removed"));
            }
            _ => insert(owners, key, src, value, false)?,
        }
    }
    let cfg = tuse_config::from_value(Json::Object(settings), src)?;
    Ok((cfg, open))
}

pub(crate) fn register(
    lua: &Lua,
    owners: Namespaces,
    cfg: &mut Config,
    open: Option<Value>,
) -> Result<Table, String> {
    let err = |e: tuse_lua::mlua::Error| e.to_string();
    let registry = lua.create_table().map_err(err)?;
    let functions = lua.create_table().map_err(err)?;
    let commands = lua.create_table().map_err(err)?;
    let hooks = lua.create_table().map_err(err)?;
    for (owner, (src, values)) in owners {
        for (section, value) in str_entries(&values, &src, Some(&owner))? {
            let path = format!("{src}: {owner}.{section}");
            if !matches!(
                section.as_str(),
                "functions" | "prompts" | "commands" | "hook"
            ) {
                return Err(format!("{path}: unknown namespace field"));
            }
            for (name, value) in str_entries(&table(value, &path)?, &path, None)? {
                let at = format!("{path}.{name}");
                if section == "prompts" {
                    if !valid_name(&name) {
                        return Err(format!("{at}: invalid prompt name"));
                    }
                    let Value::String(text) = value else {
                        return Err(format!("{at}: expected prompt text"));
                    };
                    cfg.prompts
                        .insert(format!("{owner}:{name}"), text.to_string_lossy());
                    continue;
                }
                let Value::Function(callback) = value else {
                    return Err(format!("{at}: expected a function"));
                };
                match section.as_str() {
                    "functions" => {
                        if let Some(reason) = check_function_name(&name) {
                            return Err(format!("{at}: {reason}"));
                        }
                        let key = format!("{owner}.{name}");
                        functions.raw_set(key.as_str(), callback).map_err(err)?;
                        cfg.functions.insert(key, tuse_config::FunctionKind::Custom);
                    }
                    "commands" => {
                        if !valid_name(&name) {
                            return Err(format!("{at}: invalid command name"));
                        }
                        let key = format!("{owner}:{name}");
                        commands
                            .raw_set(
                                key.as_str(),
                                lua.create_table_from([("run", callback)]).map_err(err)?,
                            )
                            .map_err(err)?;
                        cfg.commands.insert(key);
                    }
                    "hook" => {
                        if !HOOK_EVENTS.contains(&name.as_str()) {
                            return Err(format!(
                                "{at}: unknown event; expected one of {}",
                                HOOK_EVENTS.join(", ")
                            ));
                        }
                        let callbacks =
                            match hooks.raw_get::<Option<Table>>(name.as_str()).map_err(err)? {
                                Some(t) => t,
                                None => {
                                    let t = lua.create_table().map_err(err)?;
                                    hooks.raw_set(name.as_str(), t.clone()).map_err(err)?;
                                    t
                                }
                            };
                        callbacks.raw_push(callback).map_err(err)?;
                        cfg.hook.entry(name).or_default().push(owner.clone());
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    registry.raw_set("functions", functions).map_err(err)?;
    registry.raw_set("commands", commands).map_err(err)?;
    registry.raw_set("hook", hooks).map_err(err)?;
    if open.is_some() {
        cfg.open = true;
    }
    registry.raw_set("open", open).map_err(err)?;
    Ok(registry)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn lua_index(k: &Value) -> Option<usize> {
    match *k {
        Value::Integer(n) if n >= 1 => Some(n as usize),
        Value::Number(n) if n >= 1.0 && n.fract() == 0.0 && n <= usize::MAX as f64 => {
            Some(n as usize)
        }
        _ => None,
    }
}

fn to_json(v: &Value, path: &str, depth: usize) -> Result<Json, String> {
    if depth > 32 {
        return Err(format!("{path}: settings nesting is too deep"));
    }
    let err = |m: &str| Err(format!("{path}: {m}"));
    match v {
        Value::Nil => Ok(Json::Null),
        Value::Boolean(b) => Ok(Json::Bool(*b)),
        Value::Integer(n) => Ok(Json::Number((*n).into())),
        Value::Number(n) => serde_json::Number::from_f64(*n)
            .map(Json::Number)
            .ok_or_else(|| err("expected a finite number").unwrap_err()),
        Value::String(s) => Ok(Json::String(s.to_string_lossy())),
        Value::Table(t) => {
            let (mut ints, mut strs) = (BTreeMap::new(), BTreeMap::new());
            for pair in t.pairs::<Value, Value>() {
                let (k, val) = pair.map_err(|e| format!("{path}: {e}"))?;
                if let Some(i) = lua_index(&k) {
                    ints.insert(i, val);
                } else if let Value::String(s) = k {
                    strs.insert(s.to_string_lossy(), val);
                } else {
                    return err(&format!("unsupported table key `{}`", key_str(&k)));
                }
            }
            if !ints.is_empty() && !strs.is_empty() {
                return err("expected a list or string-keyed table");
            }
            if !ints.is_empty() {
                let mut arr = Vec::with_capacity(ints.len());
                for (want, (i, val)) in (1..).zip(ints) {
                    if i != want {
                        return err(&format!("list has hole at index {want}"));
                    }
                    arr.push(to_json(&val, &format!("{path}[{i}]"), depth + 1)?);
                }
                return Ok(Json::Array(arr));
            }
            let mut map = Map::new();
            for (k, val) in strs {
                map.insert(k.clone(), to_json(&val, &format!("{path}.{k}"), depth + 1)?);
            }
            Ok(Json::Object(map))
        }
        o => err(&format!("unsupported value type {}", o.type_name())),
    }
}

fn str_entries(t: &Table, src: &str, sec: Option<&str>) -> Result<Vec<(String, Value)>, String> {
    let mut out = vec![];
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair.map_err(|e| format!("{src}: {e}"))?;
        let Value::String(s) = k else {
            let ks = key_str(&k);
            let p = sec.map_or(ks.clone(), |s| format!("{s}.{ks}"));
            return Err(format!("{src}: {p}: expected string key"));
        };
        out.push((s.to_string_lossy(), v));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn key_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string_lossy(),
        Value::Integer(n) => n.to_string(),
        Value::Number(n) => n.to_string(),
        o => format!("<{}>", o.type_name()),
    }
}
