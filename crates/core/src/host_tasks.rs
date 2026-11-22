use crate::host::session;
use crate::tasks::{TaskInfo, tail};
use tuse_lua::mlua::{self, Lua, Table};

fn task(lua: &Lua, t: &TaskInfo, output: Option<String>) -> mlua::Result<Table> {
    let r = lua.create_table()?;
    r.set("id", t.id.as_str())?;
    r.set("name", t.name.as_str())?;
    r.set("status", t.status.as_str())?;
    r.set("started", t.started.as_str())?;
    r.set("ended", t.ended.as_str())?;
    r.set("output", output)?;
    Ok(r)
}

/// `task_get(id, lines)`, `task_stop(id)` and `task_list()`; `spawn` lives in the runtime.
pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    g.set(
        "task_get",
        lua.create_function(|lua, (id, lines): (String, Option<i64>)| {
            let s = session(lua)?;
            let t = s
                .tasks
                .get(&id)
                .ok_or_else(|| mlua::Error::runtime(format!("no task {id}")))?;
            let out = tail(&t.log, lines.map_or(50, |n| n.max(1) as usize));
            task(lua, &t, Some(out))
        })?,
    )?;
    g.set(
        "task_stop",
        lua.create_function(|lua, id: String| {
            let s = session(lua)?;
            let t = s.tasks.stop(&id).map_err(mlua::Error::runtime)?;
            s.emit(crate::Event::Tasks);
            task(lua, &t, None)
        })?,
    )?;
    g.set(
        "task_list",
        lua.create_function(|lua, ()| {
            let s = session(lua)?;
            let list = lua.create_table()?;
            for t in s.tasks.list() {
                list.push(task(lua, &t, None)?)?;
            }
            Ok(list)
        })?,
    )?;
    Ok(())
}
