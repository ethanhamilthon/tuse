//! `require` for configs and the agent: `tuse` (the host functions), `global`,
//! `~/path`, `path/file.lua` and modules relative to the calling file (or `cwd` for inline code);
//! `os.getenv` with Tuse's variables.

use std::path::{Path, PathBuf};
use tuse_lua::mlua::{self, Function, IntoLuaMulti, Lua, MultiValue, Table, Value};

/// The host functions `require("tuse")` gives; they stay globals too.
const HOST: &[&str] = &[
    "sh",
    "sleep",
    "spawn",
    "read",
    "write",
    "append",
    "delete",
    "edit",
    "grep",
    "docs",
    "set_title",
    "task_get",
    "task_list",
    "task_stop",
    "show_markdown",
    "show_picker",
    "show_question",
    "json",
    "inspect",
];

/// Modules loaded from files, forgotten on `/reload` so changed files load again.
const FILES: &str = "tuse.modules";
/// Canonical paths currently executing, to catch cyclic `require` calls.
const LOADING: &str = "tuse.loading";

pub(crate) fn install(lua: &Lua, cwd: &Path) -> mlua::Result<()> {
    let g = lua.globals();
    let os: Table = g.get("os")?;
    let getenv: Function = os.get("getenv")?;
    os.set(
        "getenv",
        lua.create_function(move |lua, name: String| {
            match tuse_lua::host(lua)
                .env()
                .into_iter()
                .find(|(k, _)| *k == name)
            {
                Some((_, v)) => Ok(Value::String(lua.create_string(v)?)),
                None => getenv.call::<Value>(name),
            }
        })?,
    )?;
    // `TUSE_MODE` used to be a global; point old configs at the variable.
    let meta = lua.create_table()?;
    meta.set(
        "__index",
        lua.create_function(|_, (_, k): (Value, Value)| {
            if matches!(&k, Value::String(s) if s.as_bytes().as_ref() == b"TUSE_MODE") {
                return Err::<Value, _>(mlua::Error::runtime(
                    "TUSE_MODE is an environment variable now: use os.getenv(\"TUSE_MODE\") or require(\"tuse\").env.TUSE_MODE",
                ));
            }
            Ok(Value::Nil)
        })?,
    )?;
    g.set_metatable(Some(meta))?;
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

/// Forgets the modules loaded from files, so `/reload` reads them again.
pub(crate) fn forget(lua: &Lua) -> mlua::Result<()> {
    let files: Table = lua.named_registry_value(FILES)?;
    let loaded: Table = lua.globals().get::<Table>("package")?.get("loaded")?;
    for name in files.pairs::<String, Value>() {
        loaded.set(name?.0, Value::Nil)?;
    }
    lua.set_named_registry_value(FILES, lua.create_table()?)?;
    lua.set_named_registry_value(LOADING, lua.create_table()?)
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
    let env = lua.create_table()?;
    let meta = lua.create_table()?;
    meta.set(
        "__index",
        lua.create_function(|lua, (_, k): (Table, String)| {
            let os: Table = lua.globals().get("os")?;
            os.get::<Function>("getenv")?.call::<Value>(k)
        })?,
    )?;
    env.set_metatable(Some(meta))?;
    t.set("env", env)?;
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
        if let Some(path) = src.strip_prefix('@') {
            let p = Path::new(path);
            let full = if p.is_absolute() {
                p.to_path_buf()
            } else {
                cwd.join(p)
            };
            return full
                .parent()
                .filter(|d| !d.as_os_str().is_empty())
                .map_or_else(|| cwd.to_path_buf(), Path::to_path_buf);
        }
        return cwd.to_path_buf();
    }
    cwd.to_path_buf()
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
