use crate::host::to_json;
use std::path::Path;
use tuse_config::Config;
use tuse_lua::mlua::{self, Function, Lua, Table, Value};

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
    let mut config = tuse_config::from_value(to_json(lua, Value::Table(table.clone())), &name)?;
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
    crate::require::set_loading(lua, &key, true).map_err(error)?;
    let eval = lua.load(text).set_name(format!("@{key}")).eval::<Value>();
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

fn code(t: &Table, key: &str, name: &str) -> Option<Function> {
    entry(t, key, name)?.get("run").ok()
}

fn entry(t: &Table, key: &str, name: &str) -> Option<Table> {
    t.get::<Table>(key).ok()?.get(name).ok()
}
