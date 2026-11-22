use crate::session::Session;
use crate::tools;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use tuse_lua::mlua::{self, Lua, Table, Value};
use tuse_lua::{Done, Host, TaskStart, with_run};

/// The session of a VM, filled once the session is built (the VM comes first).
pub(crate) type Slot = Arc<Mutex<Weak<Session>>>;

/// What the VM reaches in its session. Holds the session weakly: the session owns the VM.
pub(crate) struct SessionHost(pub Slot);

impl SessionHost {
    fn get(&self) -> Option<Arc<Session>> {
        self.0.lock().unwrap().upgrade()
    }
}

impl Host for SessionHost {
    fn cwd(&self) -> PathBuf {
        self.get()
            .map_or_else(|| PathBuf::from("/"), |s| s.cwd.clone())
    }

    fn env(&self) -> Vec<(String, String)> {
        self.get().map(|s| s.env()).unwrap_or_default()
    }

    fn task_start(&self, name: &str) -> Result<TaskStart, String> {
        let s = self.get().ok_or("session closed")?;
        let t = s.tasks.create(name)?;
        s.emit(crate::Event::Tasks);
        Ok(t)
    }

    fn task_end(&self, id: &str, d: Done) {
        let Some(s) = self.get() else { return };
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

fn err(e: String) -> mlua::Error {
    mlua::Error::runtime(e)
}

/// The session functions of the VM: `read`, `write`, `edit`, `grep`, `docs`, `set_title`, `task_*`.
pub(crate) fn install(lua: &Lua, s: Slot) -> mlua::Result<()> {
    lua.set_app_data(s);
    let g = lua.globals();
    g.set("read", lua.create_function(read)?)?;
    g.set(
        "write",
        lua.create_function(|lua, (path, content): (String, String)| {
            let s = session(lua)?;
            let p = tools::resolve(&path, &s.cwd);
            let (msg, diff) = tools::write(&p, &content).map_err(err)?;
            with_run(lua, |j| j.diffs.push((p.display().to_string(), diff)));
            Ok(msg)
        })?,
    )?;
    g.set("edit", lua.create_function(edit)?)?;
    g.set("grep", lua.create_function(grep)?)?;
    g.set(
        "docs",
        lua.create_function(|lua, topic: Option<String>| {
            let s = session(lua)?;
            let mut text = crate::docs::docs(topic.as_deref());
            let t = topic.unwrap_or_default().trim().to_ascii_lowercase();
            if t == "functions" || t == "functions.md" {
                text.push_str(&crate::helpers::listing(&s.config(), s.mode, true));
            }
            Ok(text)
        })?,
    )?;
    g.set(
        "set_title",
        lua.create_function(|lua, text: String| {
            let s = session(lua)?;
            let text: String = text
                .trim()
                .replace(['\r', '\n'], " ")
                .chars()
                .take(80)
                .collect();
            if text.is_empty() {
                return Err(err("empty title".into()));
            }
            let mut st = s.state.lock().unwrap();
            st.meta.title = text;
            s.save_meta(&mut st);
            Ok(())
        })?,
    )?;
    crate::host_tasks::install(lua)?;
    let names = [
        "read",
        "write",
        "edit",
        "grep",
        "docs",
        "set_title",
        "task_get",
        "task_stop",
        "task_list",
    ];
    tuse_lua::wrap(lua, &g, &names)
}

fn read(lua: &Lua, (path, o): (String, Option<Table>)) -> mlua::Result<String> {
    let s = session(lua)?;
    let p = tools::resolve(&path, &s.cwd);
    let opt = |k: &str| -> mlua::Result<Option<usize>> {
        Ok(match &o {
            Some(t) => t.get::<Option<i64>>(k)?.map(|n| n.max(1) as usize),
            None => None,
        })
    };
    let (offset, limit) = (
        opt("offset")?.unwrap_or(1),
        opt("limit")?.unwrap_or(tools::DEFAULT_LIMIT),
    );
    let r = tools::read(&p, offset, limit, s.config().settings.max_image_bytes).map_err(err)?;
    if let Some(url) = r.image {
        with_run(lua, |j| j.images.push(url));
    }
    Ok(r.text)
}

fn edit(
    lua: &Lua,
    (path, old, new, o): (String, String, String, Option<Table>),
) -> mlua::Result<String> {
    let s = session(lua)?;
    let p = tools::resolve(&path, &s.cwd);
    let all = match o {
        Some(t) => t.get::<Option<bool>>("all")?.unwrap_or(false),
        None => false,
    };
    let (msg, diff) = tools::edit(&p, &old, &new, all).map_err(err)?;
    with_run(lua, |j| j.diffs.push((p.display().to_string(), diff)));
    Ok(msg)
}

fn grep(lua: &Lua, (pattern, o): (String, Option<Table>)) -> mlua::Result<String> {
    let s = session(lua)?;
    let o = o.unwrap_or(lua.create_table()?);
    let n = |k: &str| -> mlua::Result<Option<usize>> {
        Ok(o.get::<Option<i64>>(k)?.map(|n| n.max(0) as usize))
    };
    let opts = tools::GrepOpts {
        path: o.get("path")?,
        glob: o.get("glob")?,
        ignore_case: o.get::<Option<bool>>("ignore_case")?.unwrap_or(false),
        context: n("context")?.unwrap_or(0),
        limit: n("limit")?,
    };
    let stopped = || tuse_lua::stopped(lua).is_some();
    tools::grep(&pattern, opts, &s.cwd, &stopped).map_err(err)
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
