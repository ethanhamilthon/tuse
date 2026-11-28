use tuse_lua::mlua::{self, Function, Lua, Table};

pub(crate) const FILE_ENV: &str = "tuse.file_env";
pub(crate) const AGENT_ENV: &str = "tuse.agent_env";
pub(crate) const COMMIT_AGENT: &str = "tuse.commit_agent";

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
    code_of(lua, "commands", name)
}

pub(crate) fn open_of(lua: &Lua) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    t.raw_get("open").ok()
}

pub(crate) fn hook_of(lua: &Lua, event: &str, index: usize) -> Option<Function> {
    let t: Table = lua.named_registry_value("tuse.config").ok()?;
    t.get::<Table>("hook")
        .ok()?
        .get::<Table>(event)
        .ok()?
        .get(index)
        .ok()
}
