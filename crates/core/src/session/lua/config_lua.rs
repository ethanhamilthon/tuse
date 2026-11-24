use crate::mode::{Mode, Options, Start};
use crate::scripting::registry::COMMIT_AGENT;
use std::path::Path;
use std::sync::Arc;
use tuse_config::Config;
use tuse_lua::mlua::{self, Function, Lua, Table, Value};

pub(crate) fn prepare(
    lua: &Lua,
    path: Option<&Path>,
    sync_plugins: bool,
) -> Result<Config, String> {
    let saved = crate::scripting::require::forget(lua).map_err(|e| e.to_string())?;
    let prev_ctx = lua
        .app_data_mut::<crate::session::view::snapshot::LoadCtx>()
        .map(|mut c| {
            let old = (c.loading, c.configpath.clone());
            c.loading = true;
            c.configpath = path.map(|p| p.display().to_string());
            old
        });
    let res = prepare_inner(lua, path, sync_plugins);
    if let (Some(mut ctx), Some((old_loading, old_path))) = (
        lua.app_data_mut::<crate::session::view::snapshot::LoadCtx>(),
        prev_ctx,
    ) {
        ctx.loading = old_loading;
        if res.is_err() {
            ctx.configpath = old_path;
        }
    }
    if res.is_err() {
        let _ = crate::scripting::require::restore(lua, saved);
    }
    res
}

fn prepare_inner(lua: &Lua, path: Option<&Path>, sync_plugins: bool) -> Result<Config, String> {
    crate::scripting::subs::begin_prepare(lua);
    let name = path.map_or("config".to_string(), |p| p.display().to_string());
    let (table, plugins, parsed) = match path {
        Some(p) => read_file(lua, p, sync_plugins)?,
        None => (lua.create_table().map_err(|e| e.to_string())?, vec![], None),
    };
    let mut config = match parsed {
        Some(c) => c,
        None => tuse_config::from_value(crate::scripting::validate::json(lua, &table), &name)?,
    };
    config.plugins = plugins;
    config.path = path.map(|p| p.display().to_string()).unwrap_or_default();
    lua.set_named_registry_value("tuse.next", table)
        .map_err(|e| e.to_string())?;
    crate::storage::store::apply_providers(&mut config)?;
    Ok(config)
}

fn read_file(
    lua: &Lua,
    path: &Path,
    sync_plugins: bool,
) -> Result<crate::scripting::plugins::Merged, String> {
    let source = path.display().to_string();
    let error = |e: mlua::Error| format!("{source}: {}", tuse_lua::message(&e));
    match crate::scripting::require::eval_file(lua, path)? {
        Value::Table(t) => {
            let base = path.parent().unwrap_or(Path::new("."));
            crate::scripting::plugins::apply(lua, t, &source, base, sync_plugins)
        }
        Value::Nil => Ok((lua.create_table().map_err(error)?, vec![], None)),
        _ => Err(format!("{source}: the config must return a table")),
    }
}

pub(crate) fn commit(lua: &Lua) -> Result<Option<String>, String> {
    let t: Table = lua
        .named_registry_value("tuse.next")
        .map_err(|e| e.to_string())?;
    lua.set_named_registry_value("tuse.config", &t)
        .map_err(|e| e.to_string())?;
    let commit_agent: Function = lua
        .named_registry_value(COMMIT_AGENT)
        .map_err(|e| e.to_string())?;
    let funcs: Value = t.raw_get("functions").map_err(|e| e.to_string())?;
    commit_agent
        .call::<()>(funcs)
        .map_err(|e| tuse_lua::message(&e))?;
    Ok(crate::scripting::subs::reconcile(lua))
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
) -> Result<(tuse_lua::Vm, crate::session::lua::host::Slot, Config), String> {
    let slot = crate::session::lua::host::Slot::default();
    let cwd = opts.cwd.clone();
    let mode = opts.mode;
    let host = crate::session::lua::host::SessionHost {
        slot: slot.clone(),
        cwd: cwd.clone(),
    };
    let vm = tuse_lua::Vm::new(Arc::new(host), |_| {});
    let path = tuse_config::find(opts.config.as_deref(), &cwd);
    let s2 = slot.clone();
    let config = vm.call(move |lua| {
        crate::session::lua::host::install(lua, s2).map_err(|e| tuse_lua::message(&e))?;
        lua.set_app_data(crate::session::view::snapshot::LoadCtx {
            mode,
            cwd: cwd.clone(),
            configpath: path.as_ref().map(|p| p.display().to_string()),
            loading: false,
        });
        crate::scripting::require::install(lua, &cwd).map_err(|e| tuse_lua::message(&e))?;
        let config = crate::session::lua::config_lua::prepare(lua, path.as_deref(), false)?;
        crate::session::lua::config_lua::commit(lua)?;
        Ok::<_, String>(config)
    })?;
    Ok((vm, slot, config))
}
