//! `plugins = {...}`: loads each plugin from its clone, validates it, and merges under user config.

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

fn text(t: &Table, key: &str, path: &str) -> Result<Option<String>, String> {
    match t.raw_get::<Value>(key).map_err(|e| e.to_string())? {
        Value::Nil => Ok(None),
        Value::String(s) => Ok(Some(s.to_string_lossy())),
        other => Err(format!(
            "{path}.{key}: expected string, got {}",
            crate::validate::kind(&other)
        )),
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
        other => {
            return Err(format!(
                "plugins: expected list, got {}",
                crate::validate::kind(&other)
            ));
        }
    };
    let len = list.raw_len();
    for pair in list.pairs::<Value, Value>() {
        let (k, _) = pair.map_err(|e| e.to_string())?;
        if !matches!(k, Value::Integer(i) if i >= 1 && (i as usize) <= len) {
            return Err(format!(
                "plugins: expected list (1..={len}), unexpected key {}",
                crate::validate::key_str(&k)
            ));
        }
    }
    let mut out = vec![];
    for i in 1..=len {
        let v = list.raw_get::<Value>(i).map_err(|e| e.to_string())?;
        let idx = format!("plugins[{i}]");
        let e = match v {
            Value::String(s) => Entry {
                plugin: Plugin::new(&s.to_string_lossy(), None, None, base)
                    .map_err(|e| format!("{idx}: {e}"))?,
                opts: None,
            },
            Value::Table(t) => entry(&t, &idx, base)?,
            other => {
                return Err(format!(
                    "{idx}: expected string or table, got {}",
                    crate::validate::kind(&other)
                ));
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
                "plugins: {other} and {} have conflicting ref {:?} vs {:?} for one repository",
                p.name,
                r.as_deref().unwrap_or(""),
                p.git_ref.as_deref().unwrap_or("")
            ));
        }
        refs.push((&p.root, &p.name, &p.git_ref));
    }
    Ok(out)
}

fn entry(t: &Table, idx: &str, base: &Path) -> Result<Entry, String> {
    let Value::String(name) = t.raw_get::<Value>(1).map_err(|e| e.to_string())? else {
        return Err(format!("{idx}[1]: plugin name must be a string"));
    };
    let name = name.to_string_lossy();
    for pair in t.pairs::<Value, Value>() {
        let (k, _) = pair.map_err(|e| e.to_string())?;
        match &k {
            Value::Integer(1) => {}
            Value::String(s) if KEYS.contains(&s.to_string_lossy().as_str()) => {}
            _ => {
                return Err(format!(
                    "{idx}.{}: unknown key for plugin `{name}`; use {}",
                    crate::validate::key_str(&k),
                    KEYS.join(", ")
                ));
            }
        }
    }
    let dir = text(t, "dir", idx)?;
    let mut plugin = Plugin::new(
        &name,
        dir.as_deref(),
        text(t, "path", idx)?.as_deref(),
        base,
    )
    .map_err(|e| format!("{idx}: {e}"))?;
    plugin.git_ref = text(t, "ref", idx)?;
    plugin.enabled = match t.raw_get::<Value>("enabled").map_err(|e| e.to_string())? {
        Value::Nil => true,
        Value::Boolean(b) => b,
        other => {
            return Err(format!(
                "{idx}.enabled: expected boolean, got {}",
                crate::validate::kind(&other)
            ));
        }
    };
    let opts = match t.raw_get::<Value>("opts").map_err(|e| e.to_string())? {
        Value::Nil => None,
        Value::Table(o) => Some(o),
        other => {
            return Err(format!(
                "{idx}.opts: expected table, got {}",
                crate::validate::kind(&other)
            ));
        }
    };
    Ok(Entry { plugin, opts })
}

/// The merged table, the plugins, and the parsed config when no merge happened.
pub(crate) type Merged = (
    Table,
    Vec<tuse_config::PluginInfo>,
    Option<tuse_config::Config>,
);

/// Validates the user config and each plugin, then merges plugins under the user's config.
pub(crate) fn apply(lua: &Lua, config: Table, source: &str, base: &Path) -> Result<Merged, String> {
    let parsed = crate::validate::table(lua, &config, source, false)?;
    let list = entries(&config, base).map_err(|e| format!("{source}: {e}"))?;
    let infos = list.iter().map(|e| e.plugin.info()).collect();
    if list.is_empty() {
        let _ = config.raw_set("plugins", Value::Nil);
        return Ok((config, infos, Some(parsed)));
    }
    let loaded = lua.create_table().map_err(|e| e.to_string())?;
    for e in list {
        let psrc = format!("plugin {}", e.plugin.name);
        let t = load(lua, &e).map_err(|err| format!("{psrc}: {err}"))?;
        crate::validate::table(lua, &t, &psrc, true)?;
        let item = lua.create_table().map_err(|e| e.to_string())?;
        item.set("name", e.plugin.name.as_str())
            .map_err(|e| e.to_string())?;
        item.set("config", t).map_err(|e| e.to_string())?;
        loaded.push(item).map_err(|e| e.to_string())?;
    }
    lua.load(MERGE)
        .set_name("=plugins")
        .call::<Table>((loaded, config))
        .map(|t| (t, infos, None))
        .map_err(|e| format!("{source}: {}", tuse_lua::message(&e)))
}

fn load(lua: &Lua, e: &Entry) -> Result<Table, String> {
    let p = &e.plugin;
    let Some(file) = p.entry() else {
        return Err(if p.url.is_some() && !p.root.exists() {
            format!(
                "not checked out; run `git clone {} {}`{}, see docs(\"plugins\")",
                p.url.as_deref().unwrap_or_default(),
                p.root.display(),
                p.git_ref
                    .as_deref()
                    .map(|r| format!(" and `git -C {} checkout {r}`", p.root.display()))
                    .unwrap_or_default()
            )
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
