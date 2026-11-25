use tuse_lua::mlua::{self, Function, Lua, Table};

pub(crate) const FILE_ENV: &str = "tuse.file_env";
pub(crate) const AGENT_ENV: &str = "tuse.agent_env";
pub(crate) const COMMIT_AGENT: &str = "tuse.commit_agent";

pub(crate) fn file_env(lua: &Lua) -> mlua::Result<Table> {
    lua.named_registry_value(FILE_ENV)
}

pub(crate) fn agent_env(lua: &Lua) -> mlua::Result<Table> {
    lua.named_registry_value(AGENT_ENV)
}

pub(crate) fn code_of(lua: &Lua, key: &str, name: &str) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    t.get::<Table>(key)
        .ok()?
        .get::<Table>(name)
        .ok()?
        .get("run")
        .ok()
}

pub(crate) fn command_of(lua: &Lua, name: &str) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    match t.get::<tuse_lua::mlua::Value>("commands").ok()? {
        tuse_lua::mlua::Value::Table(cmds) => match cmds.get::<tuse_lua::mlua::Value>(name).ok()? {
            tuse_lua::mlua::Value::Function(f) => Some(f),
            tuse_lua::mlua::Value::Table(c) => c.get("run").ok(),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn open_of(lua: &Lua, ext: &str) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    t.get::<Table>("settings")
        .ok()?
        .get::<Table>("open")
        .ok()?
        .get(ext)
        .ok()
}

pub(crate) fn prompt_of(lua: &Lua, name: &str) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    t.get::<Table>("prompts")
        .ok()?
        .get::<Table>(name)
        .ok()?
        .get("run")
        .ok()
}
