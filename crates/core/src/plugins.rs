//! `plugins = {...}`: loads each plugin from its clone and merges it under the user's config.

use std::path::Path;
use tuse_config::Plugin;
use tuse_lua::mlua::{Lua, Table, Value};

const MERGE: &str = include_str!("plugin_merge.lua");
const KEYS: &[&str] = &["opts", "ref", "dir", "path", "enabled"];

/// One entry of `plugins`, with the `opts` table it passes to the plugin.
pub(crate) struct Entry {
    pub plugin: Plugin,
    pub opts: Option<Table>,
}

fn text(t: &Table, key: &str, name: &str) -> Result<Option<String>, String> {
    match t.raw_get::<Value>(key).map_err(|e| e.to_string())? {
        Value::Nil => Ok(None),
        Value::String(s) => Ok(Some(s.to_string_lossy())),
        _ => Err(format!("plugin `{name}`: `{key}` must be a string")),
    }
}

/// Parses `plugins`; `base` is the config file's folder (for `dir`).
pub(crate) fn entries(config: &Table, base: &Path) -> Result<Vec<Entry>, String> {
    let list = match config
        .raw_get::<Value>("plugins")
        .map_err(|e| e.to_string())?
    {
        Value::Nil => return Ok(vec![]),
        Value::Table(t) => t,
        _ => return Err("`plugins` must be a list".into()),
    };
    let mut out = vec![];
    for v in list.sequence_values::<Value>() {
        let v = v.map_err(|e| e.to_string())?;
        let e = match v {
            Value::String(s) => Entry {
                plugin: Plugin::new(&s.to_string_lossy(), None, None, base)?,
                opts: None,
            },
            Value::Table(t) => entry(&t, base)?,
            _ => {
                return Err(
                    "plugins: each entry is \"owner/repo\" or { \"owner/repo\", ... }".into(),
                );
            }
        };
        if e.plugin.enabled {
            out.push(e);
        }
    }
    let mut refs: Vec<(&Path, &str, &Option<String>)> = vec![];
    for e in &out {
        let p = &e.plugin;
        if let Some((_, other, r)) = refs.iter().find(|(root, _, r)| {
            *root == p.root && r.is_some() && p.git_ref.is_some() && **r != p.git_ref
        }) {
            return Err(format!(
                "plugins {other} and {}: conflicting ref {:?} vs {:?} for one repository",
                p.name,
                r.as_deref().unwrap_or(""),
                p.git_ref.as_deref().unwrap_or("")
            ));
        }
        refs.push((&p.root, &p.name, &p.git_ref));
    }
    Ok(out)
}

fn entry(t: &Table, base: &Path) -> Result<Entry, String> {
    let Value::String(name) = t.raw_get::<Value>(1).map_err(|e| e.to_string())? else {
        return Err("plugins: a table entry starts with the plugin name".into());
    };
    let name = name.to_string_lossy();
    for pair in t.pairs::<Value, Value>() {
        let (k, _) = pair.map_err(|e| e.to_string())?;
        match &k {
            Value::Integer(1) => {}
            Value::String(s) if KEYS.contains(&s.to_string_lossy().as_str()) => {}
            _ => {
                return Err(format!(
                    "plugin `{name}`: unknown key {k:?}; use {}",
                    KEYS.join(", ")
                ));
            }
        }
    }
    let dir = text(t, "dir", &name)?;
    let mut plugin = Plugin::new(
        &name,
        dir.as_deref(),
        text(t, "path", &name)?.as_deref(),
        base,
    )?;
    plugin.git_ref = text(t, "ref", &name)?;
    plugin.enabled = !matches!(t.raw_get::<Value>("enabled"), Ok(Value::Boolean(false)));
    let opts = match t.raw_get::<Value>("opts").map_err(|e| e.to_string())? {
        Value::Nil => None,
        Value::Table(o) => Some(o),
        _ => return Err(format!("plugin `{name}`: `opts` must be a table")),
    };
    Ok(Entry { plugin, opts })
}

/// Loads the plugins `config` lists and returns the merged table (the user's keys on top).
pub(crate) fn apply(
    lua: &Lua,
    config: Table,
    base: &Path,
) -> Result<(Table, Vec<tuse_config::PluginInfo>), String> {
    let list = entries(&config, base)?;
    let infos = list.iter().map(|e| e.plugin.info()).collect();
    if list.is_empty() {
        let _ = config.raw_set("plugins", Value::Nil);
        return Ok((config, infos));
    }
    let loaded = lua.create_table().map_err(|e| e.to_string())?;
    for e in list {
        let t = load(lua, &e).map_err(|err| format!("plugin {}: {err}", e.plugin.name))?;
        let item = lua.create_table().map_err(|e| e.to_string())?;
        item.set("name", e.plugin.name.as_str())
            .map_err(|e| e.to_string())?;
        item.set("config", t).map_err(|e| e.to_string())?;
        loaded.push(item).map_err(|e| e.to_string())?;
    }
    lua.load(MERGE)
        .set_name("=plugins")
        .call::<Table>((loaded, config))
        .map(|t| (t, infos))
        .map_err(|e| tuse_lua::message(&e))
}

fn load(lua: &Lua, e: &Entry) -> Result<Table, String> {
    let p = &e.plugin;
    let Some(file) = p.entry() else {
        return Err(if p.url.is_some() && !p.root.exists() {
            "not installed; run `tuse plugins install`".into()
        } else {
            format!("no init.lua or tuse.lua in {}", p.dir().display())
        });
    };
    let value = crate::config_lua::eval_file(lua, &file)?;
    let value = match value {
        Value::Function(f) => {
            let opts = match &e.opts {
                Some(o) => o.clone(),
                None => lua.create_table().map_err(|e| e.to_string())?,
            };
            f.call::<Value>(opts).map_err(|e| tuse_lua::message(&e))?
        }
        v => v,
    };
    match value {
        Value::Table(t) => Ok(t),
        Value::Nil => lua.create_table().map_err(|e| e.to_string()),
        _ => Err("a plugin returns a table or function(opts) returning one".into()),
    }
}

/// The config file the session would load and the plugins it lists (plugins are not run).
pub fn configured(
    cwd: &Path,
    config: Option<&Path>,
) -> Result<Option<(std::path::PathBuf, Vec<Plugin>)>, String> {
    let Some(path) = tuse_config::find(config, cwd) else {
        return Ok(None);
    };
    let host = crate::host::SessionHost {
        slot: crate::host::Slot::default(),
        cwd: cwd.to_path_buf(),
        mode: crate::Mode::Json,
    };
    let vm = tuse_lua::Vm::new(std::sync::Arc::new(host), |_| {});
    let (slot, dir, p) = (
        crate::host::Slot::default(),
        cwd.to_path_buf(),
        path.clone(),
    );
    let list = vm.call(move |lua| {
        crate::host::install(lua, slot).map_err(|e| tuse_lua::message(&e))?;
        crate::require::install(lua, &dir).map_err(|e| tuse_lua::message(&e))?;
        let Value::Table(t) = crate::config_lua::eval_file(lua, &p)? else {
            return Ok(vec![]);
        };
        let base = p.parent().unwrap_or(Path::new("."));
        let list = entries(&t, base).map_err(|e| format!("{}: {e}", p.display()))?;
        Ok::<_, String>(list.into_iter().map(|e| e.plugin).collect())
    })?;
    Ok(Some((path, list)))
}
