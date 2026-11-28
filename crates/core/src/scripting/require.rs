use std::path::{Path, PathBuf};
use tuse_lua::mlua::{
    self, AnyUserData, Function, IntoLuaMulti, Lua, MetaMethod, MultiValue, Table, UserData,
    UserDataMethods, Value,
};

use tuse_config::{HOST_FUNCTIONS as HOST, HOST_GROUPS};

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
local orig, host_paths, host_groups, agent_env, json_proxy, docs_proxy = ...
local type, error, pairs, ipairs, rawget, rawset, setmetatable =
  type, error, pairs, ipairs, rawget, rawset, setmetatable
local sort, next = table.sort, next
local function readonly(name, values)
  return setmetatable({}, {
    __index = values,
    __newindex = function() error(name .. " is read-only", 2) end,
    __pairs = function()
      local key
      return function() local value; key, value = next(values, key); return key, value end
    end,
    __metatable = name,
  })
end
local host = {}
for _, g in ipairs(host_groups) do host[g] = readonly(g, orig[g]) end
local builtins = {}
for _, path in ipairs(host_paths) do
  local g, k = path:match("^([^.]+)%.(.+)$")
  builtins[path] = g and orig[g][k] or orig[path]
  if not g then host[path] = builtins[path] end
end
host.json, host.docs = json_proxy, docs_proxy
local keys = {}
for k, v in pairs(host) do
  keys[#keys + 1] = k
  rawset(agent_env, k, v)
end
sort(keys)
local active, roots = {}, {}
local function commit(funcs)
  for name, previous in pairs(roots) do
    if rawget(agent_env, name) == previous then rawset(agent_env, name, nil) end
  end
  active, roots = funcs, {}
  local groups = {}
  for path in pairs(funcs) do
    local owner, name = path:match("^([^.]+)%.(.+)$")
    local members = groups[owner] or {}
    groups[owner] = members
    members[name] = function(...)
      local f = active[path]
      if not f then error(path .. " is unavailable", 2) end
      return f(...)
    end
  end
  for name, members in pairs(groups) do
    local group = readonly(name, members)
    roots[name] = group
    rawset(agent_env, name, group)
  end
end
local function tuse_pairs()
  local i = 0
  return function() i = i + 1; local k = keys[i]; if k then return k, host[k] end end
end
local function json_pairs()
  local i = 0
  return function()
    i = i + 1
    if i == 1 then return "encode", orig.json.encode end
    if i == 2 then return "decode", orig.json.decode end
  end
end
return builtins, function(k) return host[k] end, orig.json.encode, orig.json.decode, tuse_pairs, json_pairs, commit
"#;

pub(crate) struct DocsObject {
    pub agent: bool,
}

fn docs_disabled(lua: &Lua, agent: bool) -> mlua::Result<bool> {
    if !agent {
        return Ok(false);
    }
    let b: Table = lua.named_registry_value(AGENT_BUILTINS)?;
    Ok(!is_truthy(&b.raw_get::<Value>("docs")?))
}

fn current_disabled(lua: &Lua) -> Vec<&'static str> {
    crate::session::lua::host::session(lua)
        .map(|s| s.config().disabled_builtins())
        .unwrap_or_default()
}

fn topics_table(lua: &Lua) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    for (i, name) in crate::docs::TOPIC_NAMES.iter().enumerate() {
        t.raw_set(i + 1, *name)?;
    }
    Ok(t)
}

impl UserData for DocsObject {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, this, key: Value| {
            if docs_disabled(lua, this.agent)? {
                return Ok(Value::Nil);
            }
            let Value::String(s) = key else {
                return Ok(Value::Nil);
            };
            let Ok(name) = s.to_str() else {
                return Ok(Value::Nil);
            };
            if name == "topics" {
                return Ok(Value::Table(topics_table(lua)?));
            }
            let disabled = current_disabled(lua);
            match crate::docs::topic(&name, &disabled) {
                Some(text) => Ok(Value::String(lua.create_string(text)?)),
                None => Ok(Value::Nil),
            }
        });
        m.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, (_k, _v): (Value, Value)| -> mlua::Result<()> {
                Err(mlua::Error::runtime("docs is read-only"))
            },
        );
        m.add_meta_method(MetaMethod::Pairs, |lua, this, ()| {
            let disabled_obj = docs_disabled(lua, this.agent)?;
            let mut entries: Vec<(String, Value)> = vec![];
            if !disabled_obj {
                entries.push(("topics".into(), Value::Table(topics_table(lua)?)));
                let disabled = current_disabled(lua);
                for name in crate::docs::TOPIC_NAMES {
                    if let Some(text) = crate::docs::topic(name, &disabled) {
                        entries.push((name.into(), Value::String(lua.create_string(text)?)));
                    }
                }
            }
            let idx = std::sync::atomic::AtomicUsize::new(0);
            let iter = lua.create_function(move |lua, ()| {
                let i = idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                match entries.get(i) {
                    Some((k, v)) => (k.as_str(), v.clone()).into_lua_multi(lua),
                    None => Value::Nil.into_lua_multi(lua),
                }
            })?;
            Ok((iter, Value::Nil, Value::Nil))
        });
    }
}

struct AgentTuse;
impl UserData for AgentTuse {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            lua.named_registry_value::<Function>("tuse.agent_tuse_get")?
                .call::<Value>(key)
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

    let (agent_orig, agent_env) = (tuse_module(lua)?, base_env(lua)?);
    // Loading an installed entrypoint runs it in the trusted environment.
    agent_orig
        .get::<Table>("plugins")?
        .raw_set("load", Value::Nil)?;
    let paths_tbl = lua.create_table()?;
    for (i, n) in HOST.iter().enumerate() {
        paths_tbl.set(i + 1, *n)?;
    }
    let groups_tbl = lua.create_table()?;
    for (i, g) in HOST_GROUPS.iter().enumerate() {
        groups_tbl.set(i + 1, *g)?;
    }
    let json_proxy: AnyUserData = lua.create_userdata(AgentJson)?;
    let docs_proxy: AnyUserData = lua.create_userdata(DocsObject { agent: true })?;
    let caps: (
        Table,
        Function,
        Function,
        Function,
        Function,
        Function,
        Function,
    ) = lua.load(AGENT_CAPS).set_name("=tuse.env").call((
        agent_orig,
        paths_tbl,
        groups_tbl,
        agent_env.clone(),
        json_proxy,
        docs_proxy,
    ))?;
    lua.set_named_registry_value(AGENT_BUILTINS, caps.0)?;
    lua.set_named_registry_value("tuse.agent_tuse_get", caps.1)?;
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
    let raw: Table = lua.named_registry_value("tuse.host_raw")?;
    let t = lua.create_table()?;
    for name in ["sh", "reload", "llm", "docs"] {
        t.raw_set(name, raw.raw_get::<Value>(name)?)?;
    }
    for group in HOST_GROUPS.iter().chain(&["json"]) {
        let sub: Table = raw.raw_get(*group)?;
        t.raw_set(*group, clone_table(lua, &sub)?)?;
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
