use crate::host::to_json;
use std::path::{Path, PathBuf};
use tuse_config::Config;
use tuse_lua::mlua::{self, Function, Lua, Table, Value};

/// Builds the config and stages its Lua functions until `commit`.
pub(crate) fn prepare(lua: &Lua, path: Option<&Path>) -> Result<Config, String> {
    crate::require::forget(lua).map_err(|e| e.to_string())?;
    let name = path.map_or("config".to_string(), |p| p.display().to_string());
    let table = match path {
        Some(p) => read_file(lua, p, &mut Vec::new())?,
        None => lua.create_table().map_err(|e| e.to_string())?,
    };
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

fn read_file(lua: &Lua, path: &Path, stack: &mut Vec<PathBuf>) -> Result<Table, String> {
    let error = |e: mlua::Error| format!("{}: {}", path.display(), tuse_lua::message(&e));
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if stack.contains(&canonical) || stack.len() >= 32 {
        return Err(format!(
            "{}: cyclic config include or too many nested includes",
            path.display()
        ));
    }
    stack.push(canonical);
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let source = match lua
        .load(text)
        .set_name(format!("@{}", path.display()))
        .eval()
        .map_err(error)?
    {
        Value::Table(t) => t,
        Value::Nil => lua.create_table().map_err(error)?,
        _ => {
            return Err(format!(
                "{}: the config must return a table",
                path.display()
            ));
        }
    };
    let out = lua.create_table().map_err(error)?;
    if let Some(includes) = source.raw_get::<Option<Table>>("include").map_err(error)? {
        let bad = || format!("{}: include must be a list of paths", path.display());
        let mut paths = Vec::new();
        for value in includes.clone().sequence_values::<Value>() {
            let Value::String(path) = value.map_err(error)? else {
                return Err(bad());
            };
            paths.push(path.to_str().map_err(error)?.to_string());
        }
        if includes.pairs::<Value, Value>().count() != paths.len() {
            return Err(bad());
        }
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        for file in paths {
            let module = read_file(lua, &tuse_config::resolve_path(&file, dir), stack)?;
            merge(lua, &out, module, "").map_err(error)?;
        }
    }
    merge(lua, &out, source, "").map_err(error)?;
    stack.pop();
    Ok(out)
}

/// Config sections merge by key, lists append, and named prompts, helpers and hooks replace.
fn merge(lua: &Lua, out: &Table, source: Table, section: &str) -> mlua::Result<()> {
    for pair in source.pairs::<String, Value>() {
        let (key, value) = pair?;
        match (section, key.as_str(), value) {
            ("", "include", _) => {}
            ("", "settings" | "theme" | "prompts" | "functions" | "hooks", Value::Table(t))
            | ("settings", "ui", Value::Table(t)) => {
                merge(lua, &section_table(lua, out, &key)?, t, &key)?;
            }
            ("", "systems" | "models", Value::Table(t)) => {
                let list = section_table(lua, out, &key)?;
                for value in t.sequence_values::<Value>() {
                    list.raw_push(value?)?;
                }
            }
            (_, _, value) => out.raw_set(key, value)?,
        }
    }
    Ok(())
}

fn section_table(lua: &Lua, out: &Table, key: &str) -> mlua::Result<Table> {
    if let Value::Table(t) = out.raw_get(key)? {
        return Ok(t);
    }
    let t = lua.create_table()?;
    out.raw_set(key, &t)?;
    Ok(t)
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
