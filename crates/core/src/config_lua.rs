use crate::host::to_json;
use std::path::Path;
use tuse_config::Config;
use tuse_lua::mlua::{self, Function, Lua, Table, Value};

const FILE_ENV: &str = "tuse.file_env";

pub(crate) fn install_env(lua: &Lua, host: &'static [&str]) -> mlua::Result<()> {
    let env = lua.create_table()?;
    env.raw_set("_G", &env)?;
    let meta = lua.create_table()?;
    meta.set(
        "__index",
        lua.create_function(move |lua, (_, key): (Table, Value)| {
            if let Value::String(s) = &key
                && host
                    .iter()
                    .any(|name| s.as_bytes().as_ref() == name.as_bytes())
            {
                return Ok(Value::Nil);
            }
            lua.globals().get(key)
        })?,
    )?;
    env.set_metatable(Some(meta))?;
    // Standard Lua loaders otherwise default to the agent's globals.
    lua.load(LOADERS)
        .set_name("=config loaders")
        .set_environment(env.clone())
        .exec()?;
    lua.set_named_registry_value(FILE_ENV, env)
}

const LOADERS: &str = r#"
local raw_load, raw_loadfile, env = load, loadfile, _G
function load(chunk, name, mode, ...)
    if select('#', ...) > 0 then return raw_load(chunk, name, mode, ...) end
    return raw_load(chunk, name, mode, env)
end
function loadfile(path, mode, ...)
    if select('#', ...) > 0 then return raw_loadfile(path, mode, ...) end
    return raw_loadfile(path, mode, env)
end
function dofile(path)
    local chunk, err = raw_loadfile(path, nil, env)
    if not chunk then error(err, 2) end
    return chunk()
end
"#;

pub(crate) fn file_env(lua: &Lua) -> mlua::Result<Table> {
    lua.named_registry_value(FILE_ENV)
}

/// Builds the config and stages its Lua functions until `commit`.
pub(crate) fn prepare(lua: &Lua, path: Option<&Path>) -> Result<Config, String> {
    crate::require::forget(lua).map_err(|e| e.to_string())?;
    let name = path.map_or("config".to_string(), |p| p.display().to_string());
    let table = match path {
        Some(p) => read_file(lua, p)?,
        None => lua.create_table().map_err(|e| e.to_string())?,
    };
    if !matches!(table.raw_get::<Value>("include"), Ok(Value::Nil)) {
        return Err(format!(
            "{name}: `include` was removed; use `require(...)` instead"
        ));
    }
    let commands = extract_commands(&table, &name)?;
    let mut raw = to_json(lua, Value::Table(table.clone()));
    if let Some(obj) = raw.as_object_mut() {
        obj.insert("commands".into(), serde_json::json!(commands));
    }
    let mut config = tuse_config::from_value(raw, &name)?;
    for (key, names) in [
        ("hooks", config.hooks.keys().cloned().collect::<Vec<_>>()),
        ("functions", config.functions.keys().cloned().collect()),
    ] {
        for n in names {
            if entry(&table, key, &n).is_some_and(|t| t.contains_key("modes").unwrap_or(false)) {
                return Err(format!(
                    "{name}: {key}.{n}.modes was removed; branch on os.getenv(\"TUSE_MODE\")"
                ));
            }
            if code(&table, key, &n).is_none() {
                return Err(format!("{name}: {key}.{n}.run must be a function"));
            }
        }
    }
    lua.set_named_registry_value("tuse.next", table)
        .map_err(|e| e.to_string())?;
    crate::store::apply_state(&mut config);
    Ok(config)
}

fn read_file(lua: &Lua, path: &Path) -> Result<Table, String> {
    let error = |e: mlua::Error| format!("{}: {}", path.display(), tuse_lua::message(&e));
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let key = canonical.display().to_string();
    let text =
        std::fs::read_to_string(&canonical).map_err(|e| format!("{}: {e}", path.display()))?;
    let env = file_env(lua).map_err(error)?;
    crate::require::set_loading(lua, &key, true).map_err(error)?;
    let eval = lua
        .load(text)
        .set_environment(env)
        .set_name(format!("@{key}"))
        .eval::<Value>();
    let _ = crate::require::set_loading(lua, &key, false);
    match eval.map_err(error)? {
        Value::Table(t) => Ok(t),
        Value::Nil => lua.create_table().map_err(error),
        _ => Err(format!(
            "{}: the config must return a table",
            path.display()
        )),
    }
}

pub(crate) fn commit(lua: &Lua, config: &Config) -> Result<(), String> {
    let t: Table = lua
        .named_registry_value("tuse.next")
        .map_err(|e| e.to_string())?;
    lua.set_named_registry_value("tuse.config", &t)
        .map_err(|e| e.to_string())?;
    for name in config.functions.keys() {
        if let Some(run) = code(&t, "functions", name) {
            lua.globals()
                .set(name.as_str(), run)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn code_of(lua: &Lua, key: &str, name: &str) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    code(&t, key, name)
}

pub(crate) fn command_of(lua: &Lua, name: &str) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    t.get::<Table>("commands").ok()?.get(name).ok()
}

fn extract_commands(table: &Table, name: &str) -> Result<Vec<String>, String> {
    let raw: Value = table.raw_get("commands").map_err(|e| e.to_string())?;
    let t = match raw {
        Value::Nil => return Ok(vec![]),
        Value::Table(t) => t,
        _ => return Err(format!("{name}: commands must be a table")),
    };
    let mut names = vec![];
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair.map_err(|e| format!("{name}: {e}"))?;
        let Value::String(s) = k else {
            return Err(format!("{name}: commands keys must be strings"));
        };
        let key = s.to_str().map_err(|e| format!("{name}: {e}"))?.to_string();
        if !matches!(v, Value::Function(_)) {
            return Err(format!("{name}: commands.{key} must be a function"));
        }
        names.push(key);
    }
    names.sort();
    Ok(names)
}

fn code(t: &Table, key: &str, name: &str) -> Option<Function> {
    entry(t, key, name)?.get("run").ok()
}

fn entry(t: &Table, key: &str, name: &str) -> Option<Table> {
    t.get::<Table>(key).ok()?.get(name).ok()
}
