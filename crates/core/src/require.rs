//! `require` for configs and the agent: `tuse` (the host functions), `global`,
//! `~/path`, `path/file.lua` and modules relative to the calling file (or `cwd` for inline code);
//! `os.getenv("THIS_PATH")` for the calling Lua file.

use std::path::{Path, PathBuf};
use tuse_lua::mlua::{self, Function, IntoLuaMulti, Lua, MultiValue, Table, Value};

use tuse_config::HOST_FUNCTIONS as HOST;

/// Modules loaded from files, forgotten on `/reload` so changed files load again.
const FILES: &str = "tuse.modules";
/// Canonical paths currently executing, to catch cyclic `require` calls.
const LOADING: &str = "tuse.loading";

/// Saved module state so a failed `/reload` can restore all previously loaded file modules.
pub(crate) struct SavedModules {
    files: Vec<(String, Value)>,
}

pub(crate) fn install(lua: &Lua, cwd: &Path) -> mlua::Result<()> {
    let g = lua.globals();
    let os: Table = g.get("os")?;
    let getenv: Function = os.get("getenv")?;
    let here = cwd.to_path_buf();
    os.set(
        "getenv",
        lua.create_function(move |lua, name: String| {
            if name == "THIS_PATH" {
                return match caller_file(lua, &here) {
                    Some(f) => Ok(Value::String(lua.create_string(f.display().to_string())?)),
                    None => Ok(Value::Nil),
                };
            }
            getenv.call::<Value>(name)
        })?,
    )?;
    crate::config_lua::install_env(lua, HOST)?;
    let package: Table = g.get("package")?;
    package.set("path", "")?;
    package.set("cpath", "")?;
    package
        .get::<Table>("loaded")?
        .set("tuse", tuse_module(lua)?)?;
    lua.set_named_registry_value(FILES, lua.create_table()?)?;
    lua.set_named_registry_value(LOADING, lua.create_table()?)?;
    let cwd = cwd.to_path_buf();
    g.set(
        "require",
        lua.create_function(move |lua, name: String| require_module(lua, name, &cwd))?,
    )?;
    tuse_lua::wrap(lua, &g, &["require"])
}

/// Forgets file-backed modules so `/reload` reads them again; returns a snapshot to roll back.
pub(crate) fn forget(lua: &Lua) -> mlua::Result<SavedModules> {
    let files: Table = lua.named_registry_value(FILES)?;
    let loaded: Table = lua.globals().get::<Table>("package")?.get("loaded")?;
    let mut saved = vec![];
    for pair in files.pairs::<String, Value>() {
        let name = pair?.0;
        let val: Value = loaded.get(name.as_str())?;
        saved.push((name.clone(), val));
        loaded.set(name, Value::Nil)?;
    }
    lua.set_named_registry_value(FILES, lua.create_table()?)?;
    lua.set_named_registry_value(LOADING, lua.create_table()?)?;
    Ok(SavedModules { files: saved })
}

/// Restores file-backed modules after a failed `/reload`.
pub(crate) fn restore(lua: &Lua, saved: SavedModules) -> mlua::Result<()> {
    let _ = forget(lua);
    let files: Table = lua.named_registry_value(FILES)?;
    let loaded: Table = lua.globals().get::<Table>("package")?.get("loaded")?;
    for (name, val) in saved.files {
        files.set(name.as_str(), true)?;
        loaded.set(name, val)?;
    }
    Ok(())
}

pub(crate) fn set_loading(lua: &Lua, key: &str, active: bool) -> mlua::Result<()> {
    let loading: Table = lua.named_registry_value(LOADING)?;
    if active {
        loading.set(key, true)
    } else {
        loading.set(key, Value::Nil)
    }
}

fn tuse_module(lua: &Lua) -> mlua::Result<Table> {
    let g = lua.globals();
    let t = lua.create_table()?;
    for name in HOST {
        t.set(*name, g.raw_get::<Value>(*name)?)?;
    }
    Ok(t)
}

fn is_truthy(v: &Value) -> bool {
    !matches!(v, Value::Nil | Value::Boolean(false))
}

fn store_loaded(loaded: &Table, key: &str, val: Value) -> mlua::Result<Value> {
    if !val.is_nil() {
        loaded.set(key, val.clone())?;
        return Ok(val);
    }
    let cur: Value = loaded.get(key)?;
    if cur.is_nil() {
        loaded.set(key, true)?;
        Ok(Value::Boolean(true))
    } else {
        Ok(cur)
    }
}

/// Directory of the Lua file calling `require`, or `cwd` for inline code.
fn caller_dir(lua: &Lua, cwd: &Path) -> PathBuf {
    caller_file(lua, cwd)
        .and_then(|f| f.parent().map(Path::to_path_buf))
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| cwd.to_path_buf())
}

/// The Lua file running the innermost Lua frame, `None` for inline code.
fn caller_file(lua: &Lua, cwd: &Path) -> Option<PathBuf> {
    for level in 0.. {
        let Some((what, source)) = lua.inspect_stack(level, |d| {
            let s = d.source();
            (s.what, s.source.map(|s| s.into_owned()))
        }) else {
            break;
        };
        let src = source.as_deref().unwrap_or("");
        if what == "C" || src == "=prelude" {
            continue;
        }
        let p = Path::new(src.strip_prefix('@')?);
        return Some(if p.is_absolute() {
            p.to_path_buf()
        } else {
            cwd.join(p)
        });
    }
    None
}

fn require_module(lua: &Lua, name: String, cwd: &Path) -> mlua::Result<MultiValue> {
    let package: Table = lua.globals().get("package")?;
    let loaded: Table = package.get("loaded")?;
    let existing: Value = loaded.get(name.as_str())?;
    if is_truthy(&existing) {
        return existing.into_lua_multi(lua);
    }
    if let Value::Function(loader) = package
        .get::<Table>("preload")?
        .get::<Value>(name.as_str())?
    {
        let val = loader
            .call::<Value>((name.as_str(), ":preload:"))
            .map_err(|e| mlua::Error::runtime(tuse_lua::message(&e)))?;
        let stored = store_loaded(&loaded, &name, val)?;
        return (stored, ":preload:").into_lua_multi(lua);
    }
    let base = caller_dir(lua, cwd);
    let tried = tuse_config::module_candidates(&name, &base);
    let Some(file) = tried.iter().find(|p| p.is_file()) else {
        let list: String = tried
            .iter()
            .map(|p| format!("\n\tno file '{}'", p.display()))
            .collect();
        return Err(mlua::Error::runtime(format!(
            "module '{name}' not found:{list}"
        )));
    };
    let canonical = file.canonicalize().unwrap_or_else(|_| file.clone());
    let key = canonical.display().to_string();
    let cached: Value = loaded.get(key.as_str())?;
    if is_truthy(&cached) {
        return cached.into_lua_multi(lua);
    }
    let loading: Table = lua.named_registry_value(LOADING)?;
    if is_truthy(&loading.get::<Value>(key.as_str())?) {
        return Err(mlua::Error::runtime(format!("cyclic require: '{key}'")));
    }
    let text = std::fs::read_to_string(&canonical)
        .map_err(|e| mlua::Error::runtime(format!("{key}: {e}")))?;
    let f = lua
        .load(text)
        .set_environment(crate::config_lua::file_env(lua)?)
        .set_name(format!("@{key}"))
        .into_function()
        .map_err(|e| mlua::Error::runtime(tuse_lua::message(&e)))?;
    loading.set(key.as_str(), true)?;
    let result = f.call::<Value>((name.as_str(), key.as_str()));
    loading.set(key.as_str(), Value::Nil)?;
    let val = result.map_err(|e| mlua::Error::runtime(tuse_lua::message(&e)))?;
    let stored = store_loaded(&loaded, &key, val)?;
    lua.named_registry_value::<Table>(FILES)?
        .set(key.as_str(), true)?;
    (stored, key).into_lua_multi(lua)
}
