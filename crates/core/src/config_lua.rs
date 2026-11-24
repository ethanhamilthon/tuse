use crate::mode::{Mode, Options, Start};
use std::path::Path;
use std::sync::Arc;
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

pub(crate) fn prepare(lua: &Lua, path: Option<&Path>) -> Result<Config, String> {
    let saved = crate::require::forget(lua).map_err(|e| e.to_string())?;
    let prev_ctx = lua.app_data_mut::<crate::snapshot::LoadCtx>().map(|mut c| {
        let old = (c.loading, c.configpath.clone());
        c.loading = true;
        c.configpath = path.map(|p| p.display().to_string());
        old
    });
    let res = prepare_inner(lua, path);
    if let (Some(mut ctx), Some((old_loading, old_path))) =
        (lua.app_data_mut::<crate::snapshot::LoadCtx>(), prev_ctx)
    {
        ctx.loading = old_loading;
        if res.is_err() {
            ctx.configpath = old_path;
        }
    }
    if res.is_err() {
        let _ = crate::require::restore(lua, saved);
    }
    res
}

fn prepare_inner(lua: &Lua, path: Option<&Path>) -> Result<Config, String> {
    let name = path.map_or("config".to_string(), |p| p.display().to_string());
    let (table, plugins, parsed) = match path {
        Some(p) => read_file(lua, p)?,
        None => (lua.create_table().map_err(|e| e.to_string())?, vec![], None),
    };
    let mut config = match parsed {
        Some(c) => c,
        None => tuse_config::from_value(crate::validate::json(lua, &table), &name)?,
    };
    config.plugins = plugins;
    config.path = path.map(|p| p.display().to_string()).unwrap_or_default();
    lua.set_named_registry_value("tuse.next", table)
        .map_err(|e| e.to_string())?;
    crate::store::apply_providers(&mut config)?;
    Ok(config)
}

fn read_file(lua: &Lua, path: &Path) -> Result<crate::plugins::Merged, String> {
    let source = path.display().to_string();
    let error = |e: mlua::Error| format!("{source}: {}", tuse_lua::message(&e));
    match eval_file(lua, path)? {
        Value::Table(t) => {
            let base = path.parent().unwrap_or(Path::new("."));
            crate::plugins::apply(lua, t, &source, base)
        }
        Value::Nil => Ok((lua.create_table().map_err(error)?, vec![], None)),
        _ => Err(format!("{source}: the config must return a table")),
    }
}

pub(crate) fn eval_file(lua: &Lua, path: &Path) -> Result<Value, String> {
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
    eval.map_err(error)
}

pub(crate) fn commit(lua: &Lua, config: &Config) -> Result<(), String> {
    let t: Table = lua
        .named_registry_value("tuse.next")
        .map_err(|e| e.to_string())?;
    if let Ok(old) = lua.named_registry_value::<Table>("tuse.config") {
        for name in table_keys(&old, "functions")? {
            if config.functions.contains_key(&name) {
                continue;
            }
            let Some(run) = code(&old, "functions", &name) else {
                continue;
            };
            let g = lua.globals();
            if g.raw_get::<Value>(name.as_str()).ok() == Some(Value::Function(run)) {
                g.raw_set(name.as_str(), Value::Nil)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
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

pub(crate) fn open_of(lua: &Lua, ext: &str) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    t.get::<Table>("open").ok()?.get(ext).ok()
}
fn table_keys(table: &Table, key: &str) -> Result<Vec<String>, String> {
    let Value::Table(t) = table.raw_get::<Value>(key).map_err(|e| e.to_string())? else {
        return Ok(vec![]);
    };
    let mut names = vec![];
    for pair in t.pairs::<String, Value>() {
        names.push(pair.map_err(|e| e.to_string())?.0);
    }
    names.sort();
    Ok(names)
}

fn code(t: &Table, key: &str, name: &str) -> Option<Function> {
    t.get::<Table>(key)
        .ok()?
        .get::<Table>(name)
        .ok()?
        .get("run")
        .ok()
}

pub fn config_arg(cwd: &Path, path: Option<&str>) -> Option<std::path::PathBuf> {
    path.map(|p| tuse_config::resolve_path(p, cwd))
}

pub fn configured_models(
    cwd: std::path::PathBuf,
    config: Option<std::path::PathBuf>,
) -> Result<Vec<tuse_config::ModelDef>, String> {
    let (_, _, config) = start_vm(&Options {
        cwd,
        config,
        mode: Mode::Tui,
        resume: None,
        start: Start::Launch,
    })?;
    Ok(config.models)
}

pub(crate) fn start_vm(
    opts: &Options,
) -> Result<(tuse_lua::Vm, crate::host::Slot, Config), String> {
    let slot = crate::host::Slot::default();
    let cwd = opts.cwd.clone();
    let mode = opts.mode;
    let host = crate::host::SessionHost {
        slot: slot.clone(),
        cwd: cwd.clone(),
    };
    let vm = tuse_lua::Vm::new(Arc::new(host), |_| {});
    let path = tuse_config::find(opts.config.as_deref(), &cwd);
    let s2 = slot.clone();
    let config = vm.call(move |lua| {
        crate::host::install(lua, s2).map_err(|e| tuse_lua::message(&e))?;
        lua.set_app_data(crate::snapshot::LoadCtx {
            mode,
            cwd: cwd.clone(),
            configpath: path.as_ref().map(|p| p.display().to_string()),
            loading: false,
        });
        crate::require::install(lua, &cwd).map_err(|e| tuse_lua::message(&e))?;
        let config = crate::config_lua::prepare(lua, path.as_deref())?;
        crate::config_lua::commit(lua, &config)?;
        Ok::<_, String>(config)
    })?;
    Ok((vm, slot, config))
}
