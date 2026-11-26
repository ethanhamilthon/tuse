use tuse_lua::mlua::{Lua, Table, Value};

pub(crate) fn table(lua: &Lua, t: &Table, source: &str) -> Result<tuse_config::Config, String> {
    let fail = |path: String, msg: &str| Err(format!("{source}: {path}: {msg}"));
    if !t
        .raw_get::<Value>("plugins")
        .map_err(|e| e.to_string())?
        .is_nil()
    {
        return fail("plugins".into(), "unknown key");
    }
    for (key, msg) in [
        ("settings", "moved to ui, history, and open"),
        ("theme", "moved to ui.theme"),
    ] {
        if !t.raw_get::<Value>(key).map_err(|e| e.to_string())?.is_nil() {
            return fail(key.into(), msg);
        }
    }
    validate_functions(t, source)?;
    for (name, v) in entries(t, "hooks") {
        let Value::Table(entry) = v else { continue };
        match entry.raw_get::<Value>("run") {
            Ok(Value::Function(_)) => {}
            Ok(Value::Nil) => return fail(format!("hooks.{name}.run"), "is required"),
            Ok(other) => return fail(format!("hooks.{name}.run"), &expected(&other)),
            Err(e) => return fail(format!("hooks.{name}.run"), &e.to_string()),
        }
    }
    for (name, v) in entries(t, "commands") {
        match v {
            Value::Table(c) => validate_command(&c)
                .map_err(|(k, m)| format!("{source}: commands.{name}{k}: {m}"))?,
            _ => return fail(format!("commands.{name}"), "expected a table"),
        }
    }
    for (name, v) in entries(t, "open") {
        if !matches!(v, Value::Function(_)) {
            return fail(format!("open.{name}"), &expected(&v));
        }
    }
    for (name, v) in entries(t, "prompts") {
        if let Value::Table(p) = v {
            validate_prompt(&p).map_err(|(k, m)| format!("{source}: prompts.{name}{k}: {m}"))?;
        }
    }
    tuse_config::from_value(json(lua, t), source)
}

fn sub(t: &Table, key: &str) -> Option<Table> {
    match t.raw_get::<Value>(key) {
        Ok(Value::Table(s)) => Some(s),
        _ => None,
    }
}

fn validate_prompt(p: &Table) -> Result<(), (String, String)> {
    validate_entry(p, &["args", "run"])
}

fn validate_command(c: &Table) -> Result<(), (String, String)> {
    match c.raw_get::<Value>("desc") {
        Ok(Value::Nil | Value::String(_)) => {}
        Ok(_) => return Err((".desc".into(), "expected a string".into())),
        Err(e) => return Err((".desc".into(), e.to_string())),
    }
    validate_entry(c, &["args", "desc", "run"])
}

fn validate_entry(p: &Table, allowed: &[&str]) -> Result<(), (String, String)> {
    for pair in p.pairs::<Value, Value>().flatten() {
        let k = key_str(&pair.0);
        if !allowed.contains(&k.as_str()) {
            return Err((format!(".{k}"), "unknown key".into()));
        }
    }
    match p.raw_get::<Value>("run") {
        Ok(Value::Function(_)) => {}
        Ok(Value::Nil) => return Err((".run".into(), "is required".into())),
        Ok(other) => return Err((".run".into(), expected(&other))),
        Err(e) => return Err((".run".into(), e.to_string())),
    }
    let args = match p.raw_get::<Value>("args") {
        Ok(Value::Nil) => return Ok(()),
        Ok(Value::Table(a)) => a,
        Ok(other) => {
            return Err((
                ".args".into(),
                format!("expected list of strings, got {}", other.type_name()),
            ));
        }
        Err(e) => return Err((".args".into(), e.to_string())),
    };
    let mut seen = vec![];
    for (i, v) in args.sequence_values::<Value>().enumerate() {
        let at = format!(".args[{}]", i + 1);
        let s = match v {
            Ok(Value::String(s)) => s.to_string_lossy(),
            Ok(other) => return Err((at, format!("expected string, got {}", other.type_name()))),
            Err(e) => return Err((at, e.to_string())),
        };
        if s.is_empty() {
            return Err((at, "must not be empty".into()));
        }
        if s.chars().any(char::is_whitespace) {
            return Err((at, "must not contain whitespace".into()));
        }
        if seen.contains(&s) {
            return Err((at, format!("duplicate arg `{s}`")));
        }
        seen.push(s);
    }
    Ok(())
}

fn validate_functions(t: &Table, source: &str) -> Result<(), String> {
    let Ok(raw) = t.raw_get::<Value>("functions") else {
        return Ok(());
    };
    match raw {
        Value::Table(_) => {}
        Value::Nil => return Ok(()),
        other => {
            return Err(format!(
                "{source}: functions: expected table, got {}",
                other.type_name()
            ));
        }
    }
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
                if !tuse_config::HOST_FUNCTIONS.contains(&name.as_str()) {
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

fn json(lua: &Lua, t: &Table) -> serde_json::Value {
    let mut raw = to_json(lua, Value::Table(t.clone()));
    if let Some(obj) = raw.as_object_mut() {
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
        if t.raw_get::<Value>("commands").is_ok_and(|v| v.is_table()) {
            let mut map = serde_json::Map::new();
            for (name, v) in entries(t, "commands") {
                map.insert(name, command_json(lua, v));
            }
            obj.insert("commands".into(), serde_json::Value::Object(map));
        }
        if sub(t, "open").is_some() {
            let mut names: Vec<String> = entries(t, "open").into_iter().map(|e| e.0).collect();
            names.sort();
            obj.insert("open".into(), serde_json::json!(names));
        }
        if sub(t, "prompts").is_some() {
            let mut map = serde_json::Map::new();
            for (name, v) in entries(t, "prompts") {
                let p = match v {
                    Value::Table(p) => {
                        let args = match p.raw_get::<Value>("args") {
                            Ok(a @ Value::Table(_)) => to_json(lua, a),
                            _ => serde_json::json!([]),
                        };
                        serde_json::json!({"kind": "function", "args": args})
                    }
                    other => to_json(lua, other),
                };
                map.insert(name, p);
            }
            obj.insert("prompts".into(), serde_json::Value::Object(map));
        }
    }
    raw
}

fn command_json(lua: &Lua, v: Value) -> serde_json::Value {
    let params = |f: &tuse_lua::mlua::Function| {
        let info = f.info();
        (!info.is_vararg).then_some(info.num_params as usize)
    };
    match v {
        Value::Table(c) => {
            let desc = match c.raw_get::<Value>("desc") {
                Ok(Value::String(s)) => s.to_string_lossy(),
                _ => String::new(),
            };
            let args = match c.raw_get::<Value>("args") {
                Ok(a @ Value::Table(_)) => to_json(lua, a),
                _ => serde_json::json!([]),
            };
            let params = match c.raw_get::<Value>("run") {
                Ok(Value::Function(f)) => params(&f),
                _ => None,
            };
            serde_json::json!({ "desc": desc, "args": args, "params": params })
        }
        _ => serde_json::Value::Null,
    }
}

fn key_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string_lossy(),
        Value::Integer(n) => n.to_string(),
        Value::Number(n) => n.to_string(),
        other => format!("<{}>", other.type_name()),
    }
}

fn to_json(lua: &Lua, v: Value) -> serde_json::Value {
    use tuse_lua::mlua::LuaSerdeExt;
    let o = tuse_lua::mlua::serde::DeserializeOptions::new().deny_unsupported_types(false);
    lua.from_value_with(v, o).unwrap_or(serde_json::Value::Null)
}
