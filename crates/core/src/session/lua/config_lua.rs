use crate::mode::{Mode, Options, Start};
use crate::scripting::registry::COMMIT_AGENT;
use crate::scripting::require::{self, SavedModules};
use crate::scripting::validate::{self, Contribution};
use crate::session::view::snapshot::LoadCtx;
use crate::storage::Providers;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tuse_config::{Config, Connect, PluginSummary};
use tuse_lua::mlua::{self, Function, Lua, Table, Value};

struct Prepared {
    snapshot: Snapshot,
    table: Table,
    saved: SavedModules,
}

#[derive(Clone)]
struct Snapshot {
    contributions: Vec<Contribution>,
    path: String,
    plugins: Vec<PluginSummary>,
    saved_providers: Vec<Connect>,
    lua_providers: BTreeMap<String, Connect>,
}

pub(crate) fn prepare(
    lua: &Lua,
    explicit: Option<&Path>,
    cwd: &Path,
    namespace: Option<&str>,
) -> Result<Config, String> {
    let active = namespace
        .map(|name| {
            let active = lua.app_data_ref::<Snapshot>().expect("active config");
            if !active
                .contributions
                .iter()
                .any(|c| c.namespace.as_deref() == Some(name))
            {
                return Err(format!(
                    "unknown active namespace `{name}`; use a full reload"
                ));
            }
            Ok((name, (*active).clone()))
        })
        .transpose()?;
    let saved = require::forget(lua).map_err(|e| e.to_string())?;
    let primary = tuse_config::find_primary(explicit, cwd);
    let init_path = primary
        .clone()
        .or_else(|| tuse_config::global_config_override(cwd))
        .or_else(|| tuse_config::global_target(cwd).ok().flatten())
        .map(|p| p.display().to_string());
    let prev = lua.app_data_mut::<LoadCtx>().map(|mut c| {
        let old = (c.loading, c.config_path.clone());
        (c.loading, c.config_path) = (true, init_path);
        old
    });
    let res = prepare_inner(lua, primary.as_deref(), cwd, active);
    if let (Some(mut ctx), Some((old_l, old_p))) = (lua.app_data_mut::<LoadCtx>(), prev) {
        ctx.loading = old_l;
        ctx.config_path = old_p;
    }
    match res {
        Ok((config, snapshot, table)) => {
            lua.set_app_data(Prepared {
                snapshot,
                table,
                saved,
            });
            Ok(config)
        }
        Err(e) => {
            let _ = require::restore(lua, saved);
            Err(e)
        }
    }
}

fn prepare_inner(
    lua: &Lua,
    primary: Option<&Path>,
    cwd: &Path,
    active: Option<(&str, Snapshot)>,
) -> Result<(Config, Snapshot, Table), String> {
    let only = active.as_ref().map(|(name, _)| *name);
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
            let (g, p, ns) = validate::extract_namespaces(&tbl, &src, true, only)?;
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
                let (_, p, ns) = validate::extract_namespaces(&gt, &gsrc, false, only)?;
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
                    namespace: Some(ns),
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
        if only.is_some() {
            continue;
        }
        let json = validate::validate_contribution(&table, &src, None)?;
        plug_contribs.push(Contribution {
            namespace: None,
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
    let mut snapshot = match active {
        Some((name, snapshot)) => snapshot.replace_namespace(name, contributions)?,
        None => Snapshot {
            contributions,
            path: primary_src.or(global_path).unwrap_or_default(),
            plugins: active_plugins,
            saved_providers: Providers::list()?
                .iter()
                .map(Providers::resolve)
                .collect::<Result<_, _>>()?,
            lua_providers: BTreeMap::new(),
        },
    };
    let (config, merged_lua) = snapshot.merge(lua)?;
    Ok((config, snapshot, merged_lua))
}

impl Snapshot {
    fn replace_namespace(mut self, name: &str, fresh: Vec<Contribution>) -> Result<Self, String> {
        let selected = |c: &Contribution| c.namespace.as_deref() == Some(name);
        for old in self.contributions.iter().filter(|c| selected(c)) {
            if !fresh.iter().any(|c| c.layer == old.layer) {
                return Err(format!(
                    "namespace `{name}` is missing from `{}`; use a full reload",
                    old.source
                ));
            }
        }
        for c in &fresh {
            if let Some(providers) = c.json.get("providers").and_then(|p| p.as_object()) {
                for key in providers.keys() {
                    self.lua_providers.remove(key);
                }
            }
        }
        self.contributions.retain(|c| !selected(c));
        self.contributions.extend(fresh);
        self.contributions.sort_by(|a, b| {
            let order = |c: &Contribution| match c.layer {
                1 => 0,
                0 => 1,
                _ => 2,
            };
            order(a)
                .cmp(&order(b))
                .then_with(|| a.namespace.cmp(&b.namespace))
        });
        Ok(self)
    }

    fn merge(&mut self, lua: &Lua) -> Result<(Config, Table), String> {
        let src = if self.path.is_empty() {
            "config"
        } else {
            &self.path
        };
        let (mut config, table) = validate::merge_contributions(lua, &self.contributions, src)?;
        config.plugins = self.plugins.clone();
        config.path = self.path.clone();
        Providers::apply(&mut config, &self.saved_providers, &self.lua_providers)?;
        self.lua_providers = config.connect[self.saved_providers.len()..]
            .iter()
            .map(|p| (p.name.clone(), p.clone()))
            .collect();
        Ok((config, table))
    }
}

pub(crate) fn commit(
    lua: &Lua,
    publish: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let Prepared {
        snapshot,
        table,
        saved,
    } = lua.remove_app_data::<Prepared>().expect("prepared config");
    let old: Option<Table> = lua
        .named_registry_value("tuse.config")
        .map_err(|e| e.to_string())?;
    if let Err(e) = apply_lua(lua, table).and_then(|()| publish()) {
        if let Some(old) = old {
            let _ = apply_lua(lua, old);
        }
        let _ = require::restore(lua, saved);
        return Err(e);
    }
    if let Some(mut ctx) = lua.app_data_mut::<LoadCtx>() {
        ctx.config_path = (!snapshot.path.is_empty()).then(|| snapshot.path.clone());
    }
    lua.set_app_data(snapshot);
    Ok(())
}

fn apply_lua(lua: &Lua, t: Table) -> Result<(), String> {
    let e = |x: mlua::Error| x.to_string();
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
        lua.set_app_data(LoadCtx {
            mode,
            cwd: cwd.clone(),
            config_path: None,
            loading: false,
        });
        crate::scripting::require::install(lua, &cwd).map_err(|e| tuse_lua::message(&e))?;
        let config = prepare(lua, explicit.as_deref(), &cwd, None)?;
        commit(lua, || Ok(()))?;
        Ok::<_, String>(config)
    })?;
    Ok((vm, slot, config))
}
