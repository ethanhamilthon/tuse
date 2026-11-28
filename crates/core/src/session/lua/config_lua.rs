use crate::mode::{LoadFlags, Mode, Options, Start};
use crate::scripting::registry::COMMIT_AGENT;
use crate::scripting::require::{self, SavedModules};
use crate::scripting::validate::{self, Namespaces};
use crate::session::view::snapshot::LoadCtx;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tuse_config::{Config, InstalledPlugin, PluginSummary};
use tuse_lua::mlua::{self, Function, Lua, Table, Value};

static SNAP_SEQ: AtomicU64 = AtomicU64::new(0);

struct PluginSnapshot {
    dir: PathBuf,
}

impl Drop for PluginSnapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn copy_plugin_tree(src: &Path, dst: &Path, canon_root: &Path, depth: u8) -> Result<(), String> {
    if depth > 32 {
        return Err(format!("{}: directory nesting too deep", src.display()));
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta =
            std::fs::symlink_metadata(&from).map_err(|e| format!("{}: {e}", from.display()))?;
        let ft = meta.file_type();
        if ft.is_file() {
            std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
        } else if ft.is_dir() {
            copy_plugin_tree(&from, &to, canon_root, depth + 1)?;
        } else if ft.is_symlink() {
            let target = from
                .canonicalize()
                .map_err(|e| format!("{}: {e}", from.display()))?;
            if !target.starts_with(canon_root) {
                return Err(format!("{}: symlink escapes plugin root", from.display()));
            }
            if target.is_file() {
                std::fs::copy(&target, &to).map_err(|e| format!("{}: {e}", to.display()))?;
            } else if target.is_dir() {
                copy_plugin_tree(&target, &to, canon_root, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn create_snapshot_dir() -> Result<PathBuf, String> {
    let base = std::env::temp_dir();
    std::fs::create_dir_all(&base).map_err(|e| format!("{}: {e}", base.display()))?;
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    loop {
        let seq = SNAP_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = base.join(format!("tuse-plug-snap-{pid}-{nanos}-{seq}"));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        }
    }
}

fn snapshot_plugins(
    plugins: &[&InstalledPlugin],
) -> Result<(PluginSnapshot, Vec<InstalledPlugin>), String> {
    let snap = PluginSnapshot {
        dir: create_snapshot_dir()?,
    };
    let mut out = Vec::with_capacity(plugins.len());
    for p in plugins {
        let canon_root = p
            .dir
            .canonicalize()
            .map_err(|e| format!("{}: {e}", p.dir.display()))?;
        let dst = snap.dir.join(&p.name);
        copy_plugin_tree(&p.dir, &dst, &canon_root, 0)?;
        out.push(tuse_config::load_plugin_dir(&p.name, &dst)?);
    }
    Ok((snap, out))
}

struct Prepared {
    snapshot: Snapshot,
    table: Table,
    saved: SavedModules,
}

struct Snapshot {
    path: String,
    _plugin_snapshot: Option<Arc<PluginSnapshot>>,
}

pub(crate) fn prepare(lua: &Lua, cwd: &Path, flags: LoadFlags) -> Result<Config, String> {
    let primary = if flags.no_global {
        None
    } else {
        tuse_config::global_target(cwd)?
    };
    let saved = require::forget(lua).map_err(|e| e.to_string())?;
    let init_path = primary.as_ref().map(|p| p.display().to_string());
    let prev = lua.app_data_mut::<LoadCtx>().map(|mut c| {
        let old = (c.loading, c.config_path.clone());
        (c.loading, c.config_path) = (true, init_path);
        old
    });
    let res = prepare_inner(lua, primary.as_deref(), flags);
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
    global: Option<&Path>,
    flags: LoadFlags,
) -> Result<(Config, Snapshot, Table), String> {
    let mut owners = Namespaces::new();
    let (mut config, open) = match global {
        Some(path) => {
            let mut cfg = validate::global(
                require::eval_file(lua, path)?,
                &path.display().to_string(),
                &mut owners,
            )?;
            cfg.0.path = path.display().to_string();
            cfg
        }
        None => (Config::default(), None),
    };
    let mut plugin_snapshot = None;
    if !flags.no_plugins {
        let root = tuse_config::plugins_dir();
        let selected = {
            let mut snapshot = None;
            if let Some(_lock) = tuse_config::plugins_read_lock(&root)? {
                let installed = tuse_config::discover_plugins_unlocked(&root)?;
                let active: Vec<_> = installed.iter().filter(|p| !p.manifest.lazy).collect();
                if !active.is_empty() {
                    snapshot = Some(snapshot_plugins(&active)?);
                }
            }
            snapshot
        };
        if let Some((snapshot, installed)) = selected {
            for plugin in installed {
                let src = format!("plugin:{}", plugin.name);
                let export = require::eval_file(lua, &plugin.entrypoint)?;
                validate::insert(&mut owners, plugin.name.clone(), &src, export, true)?;
                config.plugins.push(PluginSummary {
                    name: plugin.name,
                    version: plugin.manifest.version,
                });
            }
            plugin_snapshot = Some(Arc::new(snapshot));
        }
    }
    let table = validate::register(lua, owners, &mut config, open)?;
    let path = config.path.clone();
    Ok((
        config,
        Snapshot {
            path,
            _plugin_snapshot: plugin_snapshot,
        },
        table,
    ))
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

pub fn configured_models(
    cwd: std::path::PathBuf,
) -> Result<std::collections::BTreeMap<String, tuse_config::ModelDef>, String> {
    let opts = Options {
        cwd,
        mode: Mode::Tui,
        resume: None,
        start: Start::Launch,
        flags: LoadFlags::default(),
    };
    Ok(start_vm(&opts)?.2.models)
}

pub(crate) fn start_vm(
    opts: &Options,
) -> Result<(tuse_lua::Vm, crate::session::lua::host::Slot, Config), String> {
    let slot = crate::session::lua::host::Slot::default();
    let (cwd, mode, flags) = (opts.cwd.clone(), opts.mode, opts.flags);
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
        let config = prepare(lua, &cwd, flags)?;
        commit(lua, || Ok(()))?;
        Ok::<_, String>(config)
    })?;
    Ok((vm, slot, config))
}
