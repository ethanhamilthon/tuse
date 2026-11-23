//! `require` for configs and the agent: `tuse` (the host functions), `global`,
//! `~/path`, `path/file.lua` and project modules; `os.getenv` with Tuse's variables.

use std::path::Path;
use tuse_lua::mlua::{self, Function, IntoLuaMulti, Lua, MultiValue, Table, Value};

/// The host functions `require("tuse")` gives; they stay globals too.
const HOST: &[&str] = &[
    "sh",
    "sleep",
    "spawn",
    "read",
    "write",
    "edit",
    "grep",
    "docs",
    "set_title",
    "task_get",
    "task_list",
    "task_stop",
    "json",
    "inspect",
];

/// Modules loaded from files, forgotten on `/reload` so changed files load again.
const FILES: &str = "tuse.modules";

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
    let preload: Function = package.get::<Table>("searchers")?.get(1)?;
    let cwd = cwd.to_path_buf();
    let search = lua.create_function(move |lua, name: String| search(lua, &name, &cwd))?;
    package.set("searchers", lua.create_sequence_from([preload, search])?)
}

/// Forgets the modules loaded from files, so `/reload` reads them again.
pub(crate) fn forget(lua: &Lua) -> mlua::Result<()> {
    let files: Table = lua.named_registry_value(FILES)?;
    let loaded: Table = lua.globals().get::<Table>("package")?.get("loaded")?;
    for name in files.pairs::<String, Value>() {
        loaded.set(name?.0, Value::Nil)?;
    }
    lua.set_named_registry_value(FILES, lua.create_table()?)
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

/// A `package.searchers` entry: the loader and its file, or the files it tried.
fn search(lua: &Lua, name: &str, cwd: &Path) -> mlua::Result<MultiValue> {
    let tried = tuse_config::module_candidates(name, cwd);
    let Some(file) = tried.iter().find(|p| p.is_file()) else {
        let list: String = tried
            .iter()
            .map(|p| format!("\n\tno file '{}'", p.display()))
            .collect();
        return list.into_lua_multi(lua);
    };
    let text = std::fs::read_to_string(file).map_err(mlua::Error::external)?;
    let f = lua
        .load(text)
        .set_name(format!("@{}", file.display()))
        .into_function()?;
    lua.named_registry_value::<Table>(FILES)?.set(name, true)?;
    (f, file.display().to_string()).into_lua_multi(lua)
}
