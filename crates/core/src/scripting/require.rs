use std::path::{Path, PathBuf};
use tuse_lua::mlua::{
    self, AnyUserData, Function, IntoLuaMulti, Lua, MetaMethod, MultiValue, Table, UserData,
    UserDataMethods, Value,
};

use tuse_config::HOST_FUNCTIONS as HOST;

const TRUSTED_FILES: &str = "tuse.modules";
const TRUSTED_LOADING: &str = "tuse.loading";
const TRUSTED_PKG: &str = "tuse.trusted_package";
const AGENT_LOADING: &str = "tuse.agent_loading";
const AGENT_PKG: &str = "tuse.agent_package";
const AGENT_BUILTINS: &str = "tuse.agent_builtins";

const STDLIB_VALS: &str = "_VERSION assert collectgarbage error getmetatable ipairs next pairs pcall print rawequal rawget rawlen rawset select setmetatable tonumber tostring type warn xpcall";
const STDLIB_LIBS: [&str; 7] = ["coroutine", "io", "math", "os", "string", "table", "utf8"];

const LOADERS: &str = r#"
local rl, rf, env = ...
function env.load(c, n, m, ...) return select('#', ...) > 0 and rl(c, n, m, ...) or rl(c, n, m, env) end
function env.loadfile(p, m, ...) return select('#', ...) > 0 and rf(p, m, ...) or rf(p, m, env) end
function env.dofile(p) local c, e = rf(p, nil, env); if not c then error(e, 2) end; return c() end
"#;

const AGENT_CAPS: &str = r#"
local orig, host_names, agent_env, json_proxy = ...
local type, error, pairs, ipairs, rawget, rawset = type, error, pairs, ipairs, rawget, rawset
local builtins, custom, builtin_proxies, custom_proxies, prev_custom = {}, {}, {}, {}, {}
local function mk_json(k) return function(...) local j = builtins.json; if not j then error("json is disabled", 2) end; return j[k](...) end end
local json_enc, json_dec = mk_json("encode"), mk_json("decode")
for _, name in ipairs(host_names) do
    builtins[name] = orig[name]
    builtin_proxies[name] = name == "json" and json_proxy or function(...)
        local f = builtins[name]; if not f then error(name .. " is disabled", 2) end; return f(...)
    end
    rawset(agent_env, name, builtin_proxies[name])
end
local function tuse_pairs()
    local i = 0
    return function()
        while true do
            i = i + 1; local k = host_names[i]; if not k then return end
            if builtins[k] then return k, builtin_proxies[k] end
        end
    end
end
local function json_pairs()
    if not builtins.json then return function() end end
    local keys, i = { "encode", "decode" }, 0
    return function() i = i + 1; local k = keys[i]; if k then return k, k == "encode" and json_enc or json_dec end end
end
local function get_custom_proxy(name)
    local p = custom_proxies[name]
    if not p then
        p = function(...)
            local f = custom[name]
            if f == false then error(name .. " is disabled", 2) end
            if not f then error(name .. " is unavailable", 2) end
            return f(...)
        end
        custom_proxies[name] = p
    end
    return p
end
local function commit(funcs)
    funcs = funcs or {}
    for _, name in ipairs(host_names) do
        local was = builtins[name] == false or builtins[name] ~= orig[name]
        local v, proxy = funcs[name], builtin_proxies[name]
        if v == false then
            builtins[name] = false; if rawget(agent_env, name) == proxy then rawset(agent_env, name, nil) end
        elseif type(v) == "function" then
            builtins[name] = v; rawset(agent_env, name, proxy)
        else
            builtins[name] = orig[name]; if was and rawget(agent_env, name) == nil then rawset(agent_env, name, proxy) end
        end
    end
    for name in pairs(prev_custom) do
        if funcs[name] == nil then
            custom[name] = nil; if rawget(agent_env, name) == custom_proxies[name] then rawset(agent_env, name, nil) end
        end
    end
    prev_custom = {}
    for name, v in pairs(funcs) do
        if orig[name] == nil then
            prev_custom[name] = true
            if v == false then
                custom[name] = false
                if custom_proxies[name] and rawget(agent_env, name) == custom_proxies[name] then rawset(agent_env, name, nil) end
            elseif type(v) == "function" then
                custom[name] = v; rawset(agent_env, name, get_custom_proxy(name))
            end
        end
    end
end
return builtins, builtin_proxies, json_enc, json_dec, tuse_pairs, json_pairs, commit
"#;

struct AgentTuse;
impl UserData for AgentTuse {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            let b: Table = lua.named_registry_value(AGENT_BUILTINS)?;
            if is_truthy(&b.raw_get::<Value>(key.clone())?) {
                return lua
                    .named_registry_value::<Table>("tuse.agent_proxies")?
                    .raw_get(key);
            }
            Ok(Value::Nil)
        });
        m.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, (_k, _v): (Value, Value)| -> mlua::Result<()> {
                Err(mlua::Error::runtime("tuse module is read-only"))
            },
        );
        m.add_meta_method(MetaMethod::Pairs, |lua, _, ()| {
            lua.named_registry_value::<Function>("tuse.agent_tuse_pairs")?
                .call::<MultiValue>(())
        });
    }
}

struct AgentJson;
impl UserData for AgentJson {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            let b: Table = lua.named_registry_value(AGENT_BUILTINS)?;
            if !is_truthy(&b.raw_get::<Value>("json")?) {
                return Ok(Value::Nil);
            }
            match key {
                Value::String(s) if s.as_bytes() == b"encode" => {
                    lua.named_registry_value("tuse.agent_json_enc")
                }
                Value::String(s) if s.as_bytes() == b"decode" => {
                    lua.named_registry_value("tuse.agent_json_dec")
                }
                _ => Ok(Value::Nil),
            }
        });
        m.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, (_k, _v): (Value, Value)| -> mlua::Result<()> {
                Err(mlua::Error::runtime("json module is read-only"))
            },
        );
        m.add_meta_method(MetaMethod::Pairs, |lua, _, ()| {
            lua.named_registry_value::<Function>("tuse.agent_json_pairs")?
                .call::<MultiValue>(())
        });
    }
}

pub(crate) struct SavedModules {
    files: Vec<(String, Value)>,
    preload: Vec<(String, Value)>,
}

pub(crate) fn install(lua: &Lua, cwd: &Path) -> mlua::Result<()> {
    let os: Table = lua.globals().get("os")?;
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
    for k in [TRUSTED_FILES, TRUSTED_LOADING, AGENT_LOADING] {
        lua.set_named_registry_value(k, lua.create_table()?)?;
    }
    let trusted_tuse = tuse_module(lua)?;
    lua.set_named_registry_value("tuse.trusted_tuse", trusted_tuse.clone())?;
    let trusted_env = base_env(lua)?;
    let trusted_pkg = make_package(lua, Value::Table(trusted_tuse))?;
    trusted_env.raw_set("package", trusted_pkg.clone())?;
    lua.set_named_registry_value(TRUSTED_PKG, trusted_pkg)?;
    lua.set_named_registry_value(crate::scripting::registry::FILE_ENV, trusted_env.clone())?;
    install_require(lua, &trusted_env, cwd, true)?;

    let (agent_orig, agent_env, names_tbl) =
        (tuse_module(lua)?, base_env(lua)?, lua.create_table()?);
    for (i, n) in HOST.iter().enumerate() {
        names_tbl.set(i + 1, *n)?;
    }
    let json_proxy: AnyUserData = lua.create_userdata(AgentJson)?;
    let caps: (
        Table,
        Table,
        Function,
        Function,
        Function,
        Function,
        Function,
    ) = lua.load(AGENT_CAPS).set_name("=tuse.env").call((
        agent_orig,
        names_tbl,
        agent_env.clone(),
        json_proxy,
    ))?;
    lua.set_named_registry_value(AGENT_BUILTINS, caps.0)?;
    lua.set_named_registry_value("tuse.agent_proxies", caps.1)?;
    lua.set_named_registry_value("tuse.agent_json_enc", caps.2)?;
    lua.set_named_registry_value("tuse.agent_json_dec", caps.3)?;
    lua.set_named_registry_value("tuse.agent_tuse_pairs", caps.4)?;
    lua.set_named_registry_value("tuse.agent_json_pairs", caps.5)?;
    let commit_fn = caps.6;

    let agent_tuse: AnyUserData = lua.create_userdata(AgentTuse)?;
    lua.set_named_registry_value("tuse.agent_tuse", agent_tuse.clone())?;
    let agent_pkg = make_package(lua, Value::UserData(agent_tuse.clone()))?;
    agent_env.raw_set("package", agent_pkg.clone())?;
    lua.set_named_registry_value(AGENT_PKG, agent_pkg.clone())?;
    lua.set_named_registry_value(crate::scripting::registry::AGENT_ENV, agent_env.clone())?;
    let commit_wrap = lua.create_function(move |_, funcs: Value| {
        commit_fn.call::<()>(funcs)?;
        if let Ok(loaded) = agent_pkg.get::<Table>("loaded") {
            let _ = loaded.raw_set("tuse", agent_tuse.clone());
        }
        Ok(())
    })?;
    lua.set_named_registry_value(crate::scripting::registry::COMMIT_AGENT, commit_wrap)?;
    install_require(lua, &agent_env, cwd, false)
}

fn clone_table(lua: &Lua, src: &Table) -> mlua::Result<Table> {
    let dst = lua.create_table()?;
    for pair in src.pairs::<Value, Value>() {
        let (k, v) = pair?;
        dst.raw_set(k, v)?;
    }
    Ok(dst)
}

fn base_env(lua: &Lua) -> mlua::Result<Table> {
    let g = lua.globals();
    let env = lua.create_table()?;
    env.raw_set("_G", &env)?;
    for name in STDLIB_VALS.split_whitespace() {
        env.raw_set(name, g.raw_get::<Value>(name)?)?;
    }
    for name in STDLIB_LIBS {
        let lib: Table = g.raw_get(name)?;
        env.raw_set(name, clone_table(lua, &lib)?)?;
    }
    let raw_load: Function = g.get("load")?;
    let raw_loadfile: Function = g.get("loadfile")?;
    lua.load(LOADERS)
        .set_name("=tuse.env")
        .call::<()>((raw_load, raw_loadfile, env.clone()))?;
    Ok(env)
}

fn make_package(lua: &Lua, tuse: Value) -> mlua::Result<Table> {
    let pkg = lua.create_table()?;
    pkg.set("path", "")?;
    pkg.set("cpath", "")?;
    let loaded = lua.create_table()?;
    loaded.set("tuse", tuse)?;
    pkg.set("loaded", loaded)?;
    pkg.set("preload", lua.create_table()?)?;
    Ok(pkg)
}

fn install_require(lua: &Lua, env: &Table, cwd: &Path, trusted: bool) -> mlua::Result<()> {
    let cwd = cwd.to_path_buf();
    env.raw_set(
        "require",
        lua.create_function(move |lua, name: String| require_module(lua, name, &cwd, trusted))?,
    )?;
    tuse_lua::wrap(lua, env, &["require"])
}

pub(crate) fn forget(lua: &Lua) -> mlua::Result<SavedModules> {
    let files: Table = lua.named_registry_value(TRUSTED_FILES)?;
    let pkg: Table = lua.named_registry_value(TRUSTED_PKG)?;
    let loaded: Table = pkg.get("loaded")?;
    let mut saved = vec![];
    for pair in files.pairs::<String, Value>() {
        let name = pair?.0;
        saved.push((name.clone(), loaded.get(name.as_str())?));
        loaded.set(name, Value::Nil)?;
    }
    let preload: Vec<_> = pkg
        .get::<Table>("preload")?
        .pairs::<String, Value>()
        .collect::<mlua::Result<_>>()?;
    pkg.set("preload", lua.create_table()?)?;
    lua.set_named_registry_value(TRUSTED_FILES, lua.create_table()?)?;
    lua.set_named_registry_value(TRUSTED_LOADING, lua.create_table()?)?;
    Ok(SavedModules {
        files: saved,
        preload,
    })
}

pub(crate) fn restore(lua: &Lua, saved: SavedModules) -> mlua::Result<()> {
    let _ = forget(lua);
    let (files, pkg): (Table, Table) = (
        lua.named_registry_value(TRUSTED_FILES)?,
        lua.named_registry_value(TRUSTED_PKG)?,
    );
    let (loaded, preload_tbl): (Table, Table) = (pkg.get("loaded")?, lua.create_table()?);
    for (k, v) in saved.preload {
        preload_tbl.set(k, v)?;
    }
    pkg.set("preload", preload_tbl)?;
    for (name, val) in saved.files {
        files.set(name.as_str(), true)?;
        loaded.set(name, val)?;
    }
    Ok(())
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

fn caller_dir(lua: &Lua, cwd: &Path) -> PathBuf {
    caller_file(lua, cwd)
        .and_then(|f| f.parent().map(Path::to_path_buf))
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| cwd.to_path_buf())
}

fn caller_file(lua: &Lua, cwd: &Path) -> Option<PathBuf> {
    for level in 0.. {
        let Some((what, source)) = lua.inspect_stack(level, |d| {
            let s = d.source();
            (s.what, s.source.map(|s| s.into_owned()))
        }) else {
            break;
        };
        let src = source.as_deref().unwrap_or("");
        if what == "C" || src == "=prelude" || src == "=tuse.env" {
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

fn require_module(lua: &Lua, name: String, cwd: &Path, trusted: bool) -> mlua::Result<MultiValue> {
    if name == "tuse" {
        let k = if trusted {
            "tuse.trusted_tuse"
        } else {
            "tuse.agent_tuse"
        };
        return lua.named_registry_value::<Value>(k)?.into_lua_multi(lua);
    }
    let pkg_key = if trusted { TRUSTED_PKG } else { AGENT_PKG };
    let package: Table = lua.named_registry_value(pkg_key)?;
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
        if trusted {
            lua.named_registry_value::<Table>(TRUSTED_FILES)?
                .set(name.as_str(), true)?;
        }
        return (stored, ":preload:").into_lua_multi(lua);
    }
    if trusted && let Some(pname) = name.strip_prefix("plugin:") {
        let val = load_plugin_by_name(lua, pname).map_err(mlua::Error::runtime)?;
        return (val, name).into_lua_multi(lua);
    }
    if trusted && name == "plugins" {
        let tbl = load_all_plugins_merged(lua).map_err(mlua::Error::runtime)?;
        let stored = store_loaded(&loaded, "plugins", Value::Table(tbl))?;
        lua.named_registry_value::<Table>(TRUSTED_FILES)?
            .set("plugins", true)?;
        return (stored, "plugins").into_lua_multi(lua);
    }
    let base = if name == "global" {
        cwd.to_path_buf()
    } else {
        caller_dir(lua, cwd)
    };
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
    load_file_module(lua, &name, file, trusted)?.into_lua_multi(lua)
}

fn load_chunk(
    lua: &Lua,
    file: &Path,
    loading_key: &str,
    env_key: &str,
    args: Option<(&str, &str)>,
) -> mlua::Result<(Value, String)> {
    let canonical = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let key = canonical.display().to_string();
    let loading: Table = lua.named_registry_value(loading_key)?;
    if is_truthy(&loading.get::<Value>(key.as_str())?) {
        return Err(mlua::Error::runtime(format!("cyclic require: '{key}'")));
    }
    let text = std::fs::read_to_string(&canonical)
        .map_err(|e| mlua::Error::runtime(format!("{key}: {e}")))?;
    let f = lua
        .load(text)
        .set_environment(lua.named_registry_value::<Table>(env_key)?)
        .set_name(format!("@{key}"))
        .into_function()
        .map_err(|e| mlua::Error::runtime(tuse_lua::message(&e)))?;
    loading.set(key.as_str(), true)?;
    let res = match args {
        Some((n, _)) => f.call::<Value>((n, key.as_str())),
        None => f.call::<Value>(()),
    };
    loading.set(key.as_str(), Value::Nil)?;
    Ok((
        res.map_err(|e| mlua::Error::runtime(tuse_lua::message(&e)))?,
        key,
    ))
}

pub(crate) fn eval_file(lua: &Lua, path: &Path) -> Result<Value, String> {
    if !path.is_file() {
        return Err(format!("{}: file not found", path.display()));
    }
    load_chunk(
        lua,
        path,
        TRUSTED_LOADING,
        crate::scripting::registry::FILE_ENV,
        None,
    )
    .map(|(v, _)| v)
    .map_err(|e| format!("{}: {}", path.display(), tuse_lua::message(&e)))
}

pub(crate) fn load_plugin_raw(
    lua: &Lua,
    plugin: &tuse_config::InstalledPlugin,
) -> Result<Value, String> {
    let cache_key = format!("plugin:{}", plugin.name);
    let loaded: Table = lua
        .named_registry_value::<Table>(TRUSTED_PKG)
        .and_then(|p| p.get("loaded"))
        .map_err(|e| e.to_string())?;
    let existing: Value = loaded.get(cache_key.as_str()).map_err(|e| e.to_string())?;
    if is_truthy(&existing) {
        return Ok(existing);
    }
    let stored = load_file_module(lua, &cache_key, &plugin.entrypoint, true)
        .map_err(|e| tuse_lua::message(&e))?
        .0;
    let _ = store_loaded(&loaded, &cache_key, stored.clone());
    if let Ok(tf) = lua.named_registry_value::<Table>(TRUSTED_FILES) {
        let _ = tf.set(cache_key.as_str(), true);
    }
    Ok(stored)
}

pub(crate) fn eval_plugin(
    lua: &Lua,
    plugin: &tuse_config::InstalledPlugin,
    opt: Value,
) -> Result<Table, String> {
    let src = format!("plugin:{}", plugin.name);
    let val = match load_plugin_raw(lua, plugin)? {
        Value::Function(f) => f
            .call::<Value>(opt)
            .map_err(|e| format!("{src}: {}", tuse_lua::message(&e)))?,
        other => other,
    };
    match val {
        Value::Table(t) => Ok(t),
        _ => Err(format!(
            "{src}: plugin must return a table or function returning a table"
        )),
    }
}

fn load_file_module(
    lua: &Lua,
    name: &str,
    file: &Path,
    trusted: bool,
) -> mlua::Result<(Value, String)> {
    let (pkg_key, loading_key, env_key) = if trusted {
        (
            TRUSTED_PKG,
            TRUSTED_LOADING,
            crate::scripting::registry::FILE_ENV,
        )
    } else {
        (
            AGENT_PKG,
            AGENT_LOADING,
            crate::scripting::registry::AGENT_ENV,
        )
    };
    let loaded: Table = lua.named_registry_value::<Table>(pkg_key)?.get("loaded")?;
    let canonical = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let key = canonical.display().to_string();
    let cached: Value = loaded.get(key.as_str())?;
    if is_truthy(&cached) {
        return Ok((cached, key));
    }
    let (val, key) = load_chunk(lua, &canonical, loading_key, env_key, Some((name, &key)))?;
    let stored = store_loaded(&loaded, &key, val)?;
    if trusted {
        lua.named_registry_value::<Table>(TRUSTED_FILES)?
            .set(key.as_str(), true)?;
    }
    Ok((stored, key))
}

fn load_plugin_by_name(lua: &Lua, name: &str) -> Result<Value, String> {
    tuse_config::validate_plugin_name(name)?;
    let dir = tuse_config::plugins_dir().join(name);
    if !dir.is_dir() || !dir.join("tuse-plugin.json").is_file() {
        return Err(format!("plugin '{name}' not found"));
    }
    load_plugin_raw(lua, &tuse_config::load_plugin_dir(name, &dir)?)
}
fn load_all_plugins_merged(lua: &Lua) -> Result<Table, String> {
    let installed = tuse_config::discover_plugins()?;
    let mut contribs = Vec::with_capacity(installed.len());
    for p in &installed {
        let (src, table) = (
            format!("plugin:{}", p.name),
            eval_plugin(lua, p, Value::Nil)?,
        );
        let json = crate::scripting::validate::validate_contribution(&table, &src, None)?;
        contribs.push(crate::scripting::validate::Contribution {
            namespace: None,
            source: src,
            layer: 0,
            table,
            json,
        });
    }
    crate::scripting::validate::merge_plugins_lua(lua, &contribs)
}
