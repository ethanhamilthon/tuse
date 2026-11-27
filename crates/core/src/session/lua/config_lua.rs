use crate::mode::{Mode, Options, Start};
use crate::scripting::registry::COMMIT_AGENT;
use crate::scripting::validate::{self, Contribution};
use std::path::Path;
use std::sync::Arc;
use tuse_config::{Config, PluginSummary};
use tuse_lua::mlua::{self, Function, Lua, Table, Value};

pub(crate) fn prepare(lua: &Lua, explicit: Option<&Path>, cwd: &Path) -> Result<Config, String> {
    let saved = crate::scripting::require::forget(lua).map_err(|e| e.to_string())?;
    let primary = tuse_config::find_primary(explicit, cwd);
    let init_path = primary
        .clone()
        .or_else(|| tuse_config::global_config_override(cwd))
        .or_else(|| tuse_config::global_target(cwd).ok().flatten())
        .map(|p| p.display().to_string());
    let prev = lua
        .app_data_mut::<crate::session::view::snapshot::LoadCtx>()
        .map(|mut c| {
            let old = (c.loading, c.config_path.clone());
            (c.loading, c.config_path) = (true, init_path);
            old
        });
    let res = prepare_inner(lua, primary.as_deref(), cwd);
    if let (Some(mut ctx), Some((old_l, old_p))) = (
        lua.app_data_mut::<crate::session::view::snapshot::LoadCtx>(),
        prev,
    ) {
        ctx.loading = old_l;
        ctx.config_path = match &res {
            Ok(cfg) => (!cfg.path.is_empty()).then(|| cfg.path.clone()),
            Err(_) => old_p,
        };
    }
    if res.is_err() {
        let _ = crate::scripting::require::restore(lua, saved);
    }
    res
}

fn prepare_inner(lua: &Lua, primary: Option<&Path>, cwd: &Path) -> Result<Config, String> {
    let (global_opt, local_plugins, local_ns, primary_src) = match primary {
        Some(path) => {
            let src = path.display().to_string();
            let tbl = match crate::scripting::require::eval_file(lua, path)? {
                Value::Table(t) => t,
                Value::Nil => lua
                    .create_table()
                    .map_err(|e: mlua::Error| format!("{src}: {}", tuse_lua::message(&e)))?,
                _ => return Err(format!("{src}: the config must return a table")),
            };
            let (g, p, ns) = validate::extract_namespaces(&tbl, &src, true)?;
            (g, p.map(|t| (t, src.clone())), ns, Some(src))
        }
        None => (None, None, vec![], None),
    };
    let (global_path, global_plugins, global_ns) = match global_opt {
        Some(Value::Boolean(false)) => (None, None, vec![]),
        other => match tuse_config::global_target(cwd)? {
            Some(gp) => {
                let gsrc = gp.display().to_string();
                let Value::Function(func) = crate::scripting::require::eval_file(lua, &gp)? else {
                    return Err(format!(
                        "{gsrc}: global config must return a function(opts)"
                    ));
                };
                let Value::Table(gt) = func
                    .call::<Value>(other.unwrap_or(Value::Nil))
                    .map_err(|e| format!("{gsrc}: {}", tuse_lua::message(&e)))?
                else {
                    return Err(format!(
                        "{gsrc}: global config function must return a table"
                    ));
                };
                let (_, p, ns) = validate::extract_namespaces(&gt, &gsrc, false)?;
                (Some(gsrc.clone()), p.map(|t| (t, gsrc)), ns)
            }
            None => (None, None, vec![]),
        },
    };
    let installed = tuse_config::discover_plugins()?;
    let sel = match local_plugins.as_ref().or(global_plugins.as_ref()) {
        Some((tbl, src)) => Some(validate::parse_plugin_selector(tbl, src, &installed)?),
        None => None,
    };
    let mut contributions = vec![];
    for (opt_pfx, layer, list) in [
        (&global_path, 1u8, global_ns),
        (&primary_src, 2u8, local_ns),
    ] {
        if let Some(pfx) = opt_pfx {
            for (ns, table) in list {
                let json = validate::validate_contribution(&table, pfx, Some(&ns))?;
                contributions.push(Contribution {
                    source: format!("{pfx}: {ns}"),
                    layer,
                    table,
                    json,
                });
            }
        }
    }
    let mut active_plugins = vec![];
    let mut plug_contribs = vec![];
    for plugin in validate::select_plugins(&installed, sel.as_ref()) {
        let src = format!("plugin:{}", plugin.name);
        let opt = sel
            .as_ref()
            .and_then(|s| s.opts.get(&plugin.name).cloned())
            .unwrap_or(Value::Nil);
        let table = crate::scripting::require::eval_plugin(lua, plugin, opt)?;
        let json = validate::validate_contribution(&table, &src, None)?;
        plug_contribs.push(Contribution {
            source: src,
            layer: 0,
            table,
            json,
        });
        active_plugins.push(PluginSummary {
            name: plugin.name.clone(),
            version: plugin.manifest.version.clone(),
        });
    }
    let split = contributions.iter().take_while(|c| c.layer == 1).count();
    contributions.splice(split..split, plug_contribs);
    let config_path = primary_src.or(global_path).unwrap_or_default();
    let merged_src = if config_path.is_empty() {
        "config"
    } else {
        &config_path
    };
    let (mut config, merged_lua) = validate::merge_contributions(lua, &contributions, merged_src)?;
    config.plugins = active_plugins;
    config.path = config_path;
    crate::storage::Providers::apply(&mut config)?;
    lua.set_named_registry_value("tuse.next", merged_lua)
        .map_err(|e| e.to_string())?;
    Ok(config)
}

pub(crate) fn commit(lua: &Lua) -> Result<(), String> {
    let e = |x: mlua::Error| x.to_string();
    let t: Table = lua.named_registry_value("tuse.next").map_err(e)?;
    lua.set_named_registry_value("tuse.config", &t).map_err(e)?;
    lua.named_registry_value::<Function>(COMMIT_AGENT)
        .map_err(e)?
        .call::<()>(t.raw_get::<Value>("functions").map_err(e)?)
        .map_err(|e| tuse_lua::message(&e))
}

pub fn config_arg(cwd: &Path, path: Option<&str>) -> Option<std::path::PathBuf> {
    path.map(|p| tuse_config::resolve_path(p, cwd))
}

pub fn configured_models(
    cwd: std::path::PathBuf,
    config: Option<std::path::PathBuf>,
) -> Result<Vec<tuse_config::ModelDef>, String> {
    let opts = Options {
        cwd,
        config,
        mode: Mode::Tui,
        resume: None,
        start: Start::Launch,
    };
    Ok(start_vm(&opts)?.2.models)
}

pub(crate) fn start_vm(
    opts: &Options,
) -> Result<(tuse_lua::Vm, crate::session::lua::host::Slot, Config), String> {
    let slot = crate::session::lua::host::Slot::default();
    let (cwd, explicit, mode) = (opts.cwd.clone(), opts.config.clone(), opts.mode);
    let host = crate::session::lua::host::SessionHost {
        slot: slot.clone(),
        cwd: cwd.clone(),
    };
    let vm = tuse_lua::Vm::new(Arc::new(host), |_| {});
    let s2 = slot.clone();
    let config = vm.call(move |lua| {
        crate::session::lua::host::install(lua, s2).map_err(|e| tuse_lua::message(&e))?;
        lua.set_app_data(crate::session::view::snapshot::LoadCtx {
            mode,
            cwd: cwd.clone(),
            config_path: None,
            loading: false,
        });
        crate::scripting::require::install(lua, &cwd).map_err(|e| tuse_lua::message(&e))?;
        let config = prepare(lua, explicit.as_deref(), &cwd)?;
        commit(lua)?;
        Ok::<_, String>(config)
    })?;
    Ok((vm, slot, config))
}
