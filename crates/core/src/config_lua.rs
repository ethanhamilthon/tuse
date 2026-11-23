use crate::host::to_json;
use std::path::Path;
use std::sync::Arc;
use tuse_config::{Config, Mode};
use tuse_lua::mlua::{self, Function, Lua, Table, Value};

/// Loads the config and makes it the active one at once.
pub(crate) fn load(lua: &Lua, path: Option<&Path>, mode: Mode) -> Result<Config, String> {
    let c = prepare(lua, path, mode)?;
    commit(lua, &c, mode)?;
    Ok(c)
}

/// Runs the config file in the VM and builds the config from the table it returns.
/// The table waits in the registry until `commit`; the active hooks stay until then.
pub(crate) fn prepare(lua: &Lua, path: Option<&Path>, mode: Mode) -> Result<Config, String> {
    let name = path.map_or("config".to_string(), |p| p.display().to_string());
    let table = match path {
        Some(p) => {
            run_file(lua, p, mode).map_err(|e| format!("{name}: {}", tuse_lua::message(&e)))?
        }
        None => lua.create_table().map_err(|e| e.to_string())?,
    };
    let mut config = tuse_config::from_value(to_json(lua, Value::Table(table.clone())), &name)?;
    check_code(&table, &mut config, &name)?;
    lua.set_named_registry_value("tuse.next", table)
        .map_err(|e| e.to_string())?;
    crate::store::apply_state(&mut config);
    Ok(config)
}

/// Hooks and functions now come from the prepared table.
pub(crate) fn commit(lua: &Lua, config: &Config, mode: Mode) -> Result<(), String> {
    let t: Table = lua
        .named_registry_value("tuse.next")
        .map_err(|e| e.to_string())?;
    lua.set_named_registry_value("tuse.config", t)
        .map_err(|e| e.to_string())?;
    define_functions(lua, config, mode)
}

fn run_file(lua: &Lua, path: &Path, mode: Mode) -> mlua::Result<Table> {
    let src = std::fs::read_to_string(path).map_err(|e| mlua::Error::runtime(e.to_string()))?;
    let g = lua.globals();
    g.set(
        "MODE",
        match mode {
            Mode::Tui => "tui",
            Mode::Json => "json",
        },
    )?;
    if let Some(dir) = path.parent() {
        let package: Table = g.get("package")?;
        let old: String = package.get("path")?;
        let own = format!("{}/?.lua;{}/?/init.lua;", dir.display(), dir.display());
        if !old.starts_with(&own) {
            package.set("path", format!("{own}{old}"))?;
        }
    }
    let v: Value = lua
        .load(src)
        .set_name(format!("@{}", path.display()))
        .eval()?;
    match v {
        Value::Table(t) => Ok(t),
        Value::Nil => lua.create_table(),
        _ => Err(mlua::Error::runtime("the config must return a table")),
    }
}

/// Every hook needs `run`, every function `run`, and both must be Lua functions.
fn check_code(t: &Table, config: &mut Config, name: &str) -> Result<(), String> {
    for (key, names) in [
        ("hooks", config.hooks.keys().cloned().collect::<Vec<_>>()),
        ("functions", config.functions.keys().cloned().collect()),
    ] {
        for n in names {
            if code(t, key, &n).is_none() {
                return Err(format!("{name}: {key}.{n}.run must be a function"));
            }
        }
    }
    Ok(())
}

/// `config.<key>.<name>.run` of the config table in the registry.
pub(crate) fn code_of(lua: &Lua, key: &str, name: &str) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    code(&t, key, name)
}

fn code(t: &Table, key: &str, name: &str) -> Option<Function> {
    let entries: Table = t.get(key).ok()?;
    let e: Table = entries.get(name).ok()?;
    e.get::<Function>("run").ok()
}

/// Each function of the config becomes a global of the VM.
fn define_functions(lua: &Lua, config: &Config, mode: Mode) -> Result<(), String> {
    for (name, f) in &config.functions {
        if !f.matches(mode) {
            continue;
        }
        if let Some(run) = code_of(lua, "functions", name) {
            lua.globals()
                .set(name.as_str(), run)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The theme of the config for `cwd`, read in a VM of its own (for `tuse connect`).
pub fn theme(cwd: &Path) -> tuse_config::Theme {
    let path = tuse_config::find(None, cwd);
    let vm = tuse_lua::Vm::new(
        Arc::new(crate::host::SessionHost(Default::default())),
        |_| {},
    );
    vm.call(move |lua| load(lua, path.as_deref(), Mode::Tui))
        .map(|c| c.theme)
        .unwrap_or_default()
}
