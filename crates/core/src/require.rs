use std::path::{Path, PathBuf};
use tuse_lua::mlua::{
    self, AnyUserData, Function, IntoLuaMulti, Lua, MetaMethod, MultiValue, Table, UserData,
    UserDataMethods, Value,
};

use tuse_config::HOST_FUNCTIONS as HOST;

const TRUSTED_FILES: &str = "tuse.modules";
const TRUSTED_LOADING: &str = "tuse.loading";
const TRUSTED_PKG: &str = "tuse.trusted_package";
const TRUSTED_TUSE: &str = "tuse.trusted_tuse";
const AGENT_LOADING: &str = "tuse.agent_loading";
const AGENT_PKG: &str = "tuse.agent_package";
const AGENT_TUSE: &str = "tuse.agent_tuse";
const AGENT_BUILTINS: &str = "tuse.agent_builtins";
const AGENT_PROXIES: &str = "tuse.agent_proxies";
const AGENT_JSON_ENC: &str = "tuse.agent_json_enc";
const AGENT_JSON_DEC: &str = "tuse.agent_json_dec";
const AGENT_TUSE_PAIRS: &str = "tuse.agent_tuse_pairs";
const AGENT_JSON_PAIRS: &str = "tuse.agent_json_pairs";

const STDLIB_VALS: &[&str] = &[
    "_VERSION",
    "assert",
    "collectgarbage",
    "error",
    "getmetatable",
    "ipairs",
    "next",
    "pairs",
    "pcall",
    "print",
    "rawequal",
    "rawget",
    "rawlen",
    "rawset",
    "select",
    "setmetatable",
    "tonumber",
    "tostring",
    "type",
    "warn",
    "xpcall",
];

const STDLIB_LIBS: &[&str] = &["coroutine", "io", "math", "os", "string", "table", "utf8"];

const LOADERS: &str = r#"
local raw_load, raw_loadfile, env = ...
function env.load(chunk, name, mode, ...)
    if select('#', ...) > 0 then return raw_load(chunk, name, mode, ...) end
    return raw_load(chunk, name, mode, env)
end
function env.loadfile(path, mode, ...)
    if select('#', ...) > 0 then return raw_loadfile(path, mode, ...) end
    return raw_loadfile(path, mode, env)
end
function env.dofile(path)
    local chunk, err = raw_loadfile(path, nil, env)
    if not chunk then error(err, 2) end
    return chunk()
end
"#;

const AGENT_CAPS: &str = r#"
local orig, host_names, agent_env, json_proxy = ...
local type, error, pairs, ipairs, rawget, rawset = type, error, pairs, ipairs, rawget, rawset
local builtins, custom, builtin_proxies, custom_proxies = {}, {}, {}, {}
local prev_custom = {}
local json_enc = function(...)
    local j = builtins.json
    if not j then error("json is disabled", 2) end
    return j.encode(...)
end
local json_dec = function(...)
    local j = builtins.json
    if not j then error("json is disabled", 2) end
    return j.decode(...)
end
for _, name in ipairs(host_names) do
    builtins[name] = orig[name]
    if name == "json" then
        builtin_proxies.json = json_proxy
    else
        builtin_proxies[name] = function(...)
            local f = builtins[name]
            if not f then error(name .. " is disabled", 2) end
            return f(...)
        end
    end
    rawset(agent_env, name, builtin_proxies[name])
end
local function tuse_pairs()
    local i = 0
    return function()
        while true do
            i = i + 1
            local k = host_names[i]
            if not k then return end
            if builtins[k] then return k, builtin_proxies[k] end
        end
    end
end
local function json_pairs()
    if not builtins.json then return function() end end
    local keys, i = { "encode", "decode" }, 0
    return function()
        i = i + 1
        local k = keys[i]
        if k == "encode" then return k, json_enc end
        if k == "decode" then return k, json_dec end
    end
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
        local was_disabled = builtins[name] == false
        local was_override = builtins[name] ~= false and builtins[name] ~= orig[name]
        local v = funcs[name]
        local proxy = builtin_proxies[name]
        if v == false then
            builtins[name] = false
            if rawget(agent_env, name) == proxy then
                rawset(agent_env, name, nil)
            end
        elseif type(v) == "function" then
            builtins[name] = v
            rawset(agent_env, name, proxy)
        else
            builtins[name] = orig[name]
            if (was_disabled or was_override) and rawget(agent_env, name) == nil then
                rawset(agent_env, name, proxy)
            end
        end
    end
    for name in pairs(prev_custom) do
        if funcs[name] == nil then
            custom[name] = nil
            if rawget(agent_env, name) == custom_proxies[name] then
                rawset(agent_env, name, nil)
            end
        end
    end
    prev_custom = {}
    for name, v in pairs(funcs) do
        if orig[name] == nil then
            prev_custom[name] = true
            if v == false then
                custom[name] = false
                if custom_proxies[name] and rawget(agent_env, name) == custom_proxies[name] then
                    rawset(agent_env, name, nil)
                end
            elseif type(v) == "function" then
                custom[name] = v
                rawset(agent_env, name, get_custom_proxy(name))
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
            let builtins: Table = lua.named_registry_value(AGENT_BUILTINS)?;
            if is_truthy(&builtins.raw_get::<Value>(key.clone())?) {
                let proxies: Table = lua.named_registry_value(AGENT_PROXIES)?;
                return proxies.raw_get(key);
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
            let f: Function = lua.named_registry_value(AGENT_TUSE_PAIRS)?;
            f.call::<MultiValue>(())
        });
    }
}

struct AgentJson;

impl UserData for AgentJson {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            let builtins: Table = lua.named_registry_value(AGENT_BUILTINS)?;
            if !is_truthy(&builtins.raw_get::<Value>("json")?) {
                return Ok(Value::Nil);
            }
            match key {
                Value::String(s) if s.as_bytes() == b"encode" => {
                    lua.named_registry_value(AGENT_JSON_ENC)
                }
                Value::String(s) if s.as_bytes() == b"decode" => {
                    lua.named_registry_value(AGENT_JSON_DEC)
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
            let f: Function = lua.named_registry_value(AGENT_JSON_PAIRS)?;
            f.call::<MultiValue>(())
        });
    }
}

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

    lua.set_named_registry_value(TRUSTED_FILES, lua.create_table()?)?;
    lua.set_named_registry_value(TRUSTED_LOADING, lua.create_table()?)?;
    lua.set_named_registry_value(AGENT_LOADING, lua.create_table()?)?;

    let trusted_tuse = tuse_module(lua)?;
    lua.set_named_registry_value(TRUSTED_TUSE, trusted_tuse.clone())?;
    let trusted_env = base_env(lua)?;
    let trusted_pkg = make_package(lua, Value::Table(trusted_tuse))?;
    trusted_env.raw_set("package", trusted_pkg.clone())?;
    lua.set_named_registry_value(TRUSTED_PKG, trusted_pkg)?;
    lua.set_named_registry_value(crate::registry::FILE_ENV, trusted_env.clone())?;
    install_require(lua, &trusted_env, cwd, true)?;

    let agent_orig = tuse_module(lua)?;
    let agent_env = base_env(lua)?;
    let names_tbl = lua.create_table()?;
    for (i, n) in HOST.iter().enumerate() {
        names_tbl.set(i + 1, *n)?;
    }
    let json_proxy: AnyUserData = lua.create_userdata(AgentJson)?;
    let (builtins, proxies, json_enc, json_dec, tuse_pairs, json_pairs, commit_fn): (
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
    lua.set_named_registry_value(AGENT_BUILTINS, builtins)?;
    lua.set_named_registry_value(AGENT_PROXIES, proxies)?;
    lua.set_named_registry_value(AGENT_JSON_ENC, json_enc)?;
    lua.set_named_registry_value(AGENT_JSON_DEC, json_dec)?;
    lua.set_named_registry_value(AGENT_TUSE_PAIRS, tuse_pairs)?;
    lua.set_named_registry_value(AGENT_JSON_PAIRS, json_pairs)?;

    let agent_tuse: AnyUserData = lua.create_userdata(AgentTuse)?;
    lua.set_named_registry_value(AGENT_TUSE, agent_tuse.clone())?;
    let agent_pkg = make_package(lua, Value::UserData(agent_tuse.clone()))?;
    agent_env.raw_set("package", agent_pkg.clone())?;
    lua.set_named_registry_value(AGENT_PKG, agent_pkg.clone())?;
    lua.set_named_registry_value(crate::registry::AGENT_ENV, agent_env.clone())?;
    let commit_wrap = lua.create_function(move |_, funcs: Value| {
        commit_fn.call::<()>(funcs)?;
        if let Ok(loaded) = agent_pkg.get::<Table>("loaded") {
            let _ = loaded.raw_set("tuse", agent_tuse.clone());
        }
        Ok(())
    })?;
    lua.set_named_registry_value(crate::registry::COMMIT_AGENT, commit_wrap)?;
    install_require(lua, &agent_env, cwd, false)?;
    Ok(())
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
    for name in STDLIB_VALS {
        env.raw_set(*name, g.raw_get::<Value>(*name)?)?;
    }
    for name in STDLIB_LIBS {
        let lib: Table = g.raw_get(*name)?;
        env.raw_set(*name, clone_table(lua, &lib)?)?;
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
    let loaded: Table = lua
        .named_registry_value::<Table>(TRUSTED_PKG)?
        .get("loaded")?;
    let mut saved = vec![];
    for pair in files.pairs::<String, Value>() {
        let name = pair?.0;
        let val: Value = loaded.get(name.as_str())?;
        saved.push((name.clone(), val));
        loaded.set(name, Value::Nil)?;
    }
    lua.set_named_registry_value(TRUSTED_FILES, lua.create_table()?)?;
    lua.set_named_registry_value(TRUSTED_LOADING, lua.create_table()?)?;
    Ok(SavedModules { files: saved })
}

pub(crate) fn restore(lua: &Lua, saved: SavedModules) -> mlua::Result<()> {
    let _ = forget(lua);
    let files: Table = lua.named_registry_value(TRUSTED_FILES)?;
    let loaded: Table = lua
        .named_registry_value::<Table>(TRUSTED_PKG)?
        .get("loaded")?;
    for (name, val) in saved.files {
        files.set(name.as_str(), true)?;
        loaded.set(name, val)?;
    }
    Ok(())
}

pub(crate) fn set_loading(lua: &Lua, key: &str, active: bool) -> mlua::Result<()> {
    let loading: Table = lua.named_registry_value(TRUSTED_LOADING)?;
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
        let tuse: Value =
            lua.named_registry_value(if trusted { TRUSTED_TUSE } else { AGENT_TUSE })?;
        return tuse.into_lua_multi(lua);
    }
    let pkg_key = if trusted { TRUSTED_PKG } else { AGENT_PKG };
    let loading_key = if trusted {
        TRUSTED_LOADING
    } else {
        AGENT_LOADING
    };
    let env_key = if trusted {
        crate::registry::FILE_ENV
    } else {
        crate::registry::AGENT_ENV
    };
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
    let result = f.call::<Value>((name.as_str(), key.as_str()));
    loading.set(key.as_str(), Value::Nil)?;
    let val = result.map_err(|e| mlua::Error::runtime(tuse_lua::message(&e)))?;
    let stored = store_loaded(&loaded, &key, val)?;
    if trusted {
        lua.named_registry_value::<Table>(TRUSTED_FILES)?
            .set(key.as_str(), true)?;
    }
    (stored, key).into_lua_multi(lua)
}
