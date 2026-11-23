use crate::session::Session;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;
use tuse_lua::mlua::{self, Lua, Value};
use tuse_lua::{Done, Host, TaskStart};

/// The session of a VM, filled once the session is built (the VM comes first).
pub(crate) type Slot = Arc<Mutex<Weak<Session>>>;

/// What the VM reaches in its session. Holds the session weakly: the session owns the VM.
pub(crate) struct SessionHost {
    pub slot: Slot,
    pub cwd: PathBuf,
}

impl SessionHost {
    fn get(&self) -> Option<Arc<Session>> {
        self.slot.lock().unwrap().upgrade()
    }
}

impl Host for SessionHost {
    fn cwd(&self) -> PathBuf {
        self.cwd.clone()
    }

    fn action(&self, what: &str) {
        if let Some(s) = self.get() {
            action(&s, "running", what);
        }
    }

    fn task_start(&self, name: &str, from_lua_hook: bool) -> Result<TaskStart, String> {
        let s = self.get().ok_or("session closed")?;
        let t = s.tasks.create(name)?;
        if !from_lua_hook {
            let exec_id = s.next_lua_id();
            let display = if name.is_empty() {
                t.id.clone()
            } else {
                name.to_string()
            };
            s.task_lua
                .lock()
                .unwrap()
                .insert(t.id.clone(), (exec_id.clone(), Instant::now()));
            s.lua_start_hook(exec_id, "task", None, Some(display));
        }
        s.emit(crate::Event::Tasks);
        Ok(t)
    }

    fn task_end(&self, id: &str, d: Done) {
        let Some(s) = self.get() else { return };
        if let Some((exec_id, started)) = s.task_lua.lock().unwrap().remove(id) {
            s.lua_end_done(&exec_id, "task", started.elapsed(), &d);
        }
        let ended = s.tasks.end(id, d.ok);
        s.emit(crate::Event::Tasks);
        if let Some(t) = ended {
            let last = if d.ok {
                last_line(&d.text)
            } else {
                error_line(&d.text)
            };
            let what = if d.ok { "finished" } else { "failed" };
            s.submit(crate::UserInput::notification(format!(
                "<notification>Task {} ({}) {what}. Last line: {last}. Output: `task_get(\"{}\").output`.</notification>",
                t.id, t.name, t.id
            )));
        }
    }
}

pub(crate) fn session(lua: &Lua) -> mlua::Result<Arc<Session>> {
    let slot = lua
        .app_data_ref::<Slot>()
        .map(|s| s.lock().unwrap().upgrade());
    slot.flatten()
        .ok_or_else(|| mlua::Error::runtime("session closed"))
}

pub(crate) fn err(e: String) -> mlua::Error {
    mlua::Error::runtime(e)
}

/// Shows what the agent is doing in the working indicator.
pub(crate) fn action(s: &Session, verb: &str, what: &str) {
    let what: String = what.lines().next().unwrap_or("").chars().take(60).collect();
    s.set_status(format!("{verb} {what}..."));
}

/// The session functions of the VM: file helpers, `docs`, `set_title`, `session`, `task_*`.
pub(crate) fn install(lua: &Lua, s: Slot) -> mlua::Result<()> {
    lua.set_app_data(s);
    let g = lua.globals();
    crate::host_fs::install(lua)?;
    g.set(
        "docs",
        lua.create_function(|lua, topic: Option<String>| {
            let s = session(lua)?;
            let mut text = crate::docs::docs(topic.as_deref());
            let t = topic.unwrap_or_default().trim().to_ascii_lowercase();
            if t == "functions" || t == "functions.md" {
                text.push_str(&crate::helpers::listing(&s.config(), true));
            }
            Ok(text)
        })?,
    )?;
    g.set(
        "set_title",
        lua.create_function(|lua, text: String| session(lua)?.set_name(&text).map_err(err))?,
    )?;
    g.set(
        "session",
        lua.create_function(|lua, ()| crate::snapshot::lua_session(lua))?,
    )?;
    crate::host_tasks::install(lua)?;
    crate::host_ui::install(lua)?;
    let names = [
        "read",
        "write",
        "append",
        "delete",
        "edit",
        "grep",
        "docs",
        "set_title",
        "session",
        "task_get",
        "task_stop",
        "task_list",
    ];
    tuse_lua::wrap(lua, &g, &names)
}

/// The line that says what failed: the one before a traceback, else the last one.
pub(crate) fn error_line(text: &str) -> String {
    match text.find("\nstack traceback:") {
        Some(i) => last_line(&text[..i]),
        None => last_line(text),
    }
}

pub(crate) fn last_line(text: &str) -> String {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("failed")
        .to_string()
}

/// A Lua value as JSON, functions dropped.
pub(crate) fn to_json(lua: &Lua, v: Value) -> serde_json::Value {
    use mlua::LuaSerdeExt;
    let o = mlua::serde::DeserializeOptions::new().deny_unsupported_types(false);
    lua.from_value_with(v, o).unwrap_or(serde_json::Value::Null)
}
