use crate::event::UiPrompt;
use crate::session::Session;
use crate::storage::tasks::{TaskInfo, Tasks};
use crate::tools;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;
use tuse_lua::mlua::{self, Lua, Table, Value};
use tuse_lua::{Done, ExtWait, Host, TaskStart, with_run};

pub(crate) type Slot = Arc<Mutex<Weak<Session>>>;

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
            status(&s, "running", what);
        }
    }

    fn task_start(&self, name: &str, code: &str, from_lua_hook: bool) -> Result<TaskStart, String> {
        let s = self.get().ok_or("session closed")?;
        let t = s.tasks.create(name, code)?;
        if !from_lua_hook {
            self.task_track(&t.id, name);
        }
        s.emit(crate::Event::Tasks);
        Ok(t)
    }

    fn task_track(&self, id: &str, name: &str) {
        let Some(s) = self.get() else { return };
        let exec_id = s.next_lua_id();
        let display = if name.is_empty() { id } else { name };
        s.task_lua
            .lock()
            .unwrap()
            .insert(id.to_string(), (exec_id.clone(), Instant::now()));
        let code = s.tasks.get(id).map(|t| t.code).filter(|c| !c.is_empty());
        s.lua_start_hook(exec_id, "task", code, Some(display.to_string()));
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
                crate::text::last_line(&d.text)
            } else {
                crate::text::error_line(&d.text)
            };
            let what = if d.ok { "finished" } else { "failed" };
            let msg = if s.config().is_disabled("task_get") {
                format!(
                    "<task>Task {} ({}) {what}. Last line: {last}.</task>",
                    t.id, t.name
                )
            } else {
                format!(
                    "<task>Task {} ({}) {what}. Last line: {last}. Output: `task_get(\"{}\").output`.</task>",
                    t.id, t.name, t.id
                )
            };
            s.submit(crate::UserInput::notification(msg));
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

pub(crate) fn action(lua: &Lua, s: &Session, verb: &str, what: &str) {
    if with_run(lua, |j| j.background()).unwrap_or(false) {
        return;
    }
    status(s, verb, what);
}

fn status(s: &Session, verb: &str, what: &str) {
    let what: String = what.lines().next().unwrap_or("").chars().take(60).collect();
    s.set_status(format!("{verb} {what}..."));
}

pub(crate) fn install(lua: &Lua, s: Slot) -> mlua::Result<()> {
    lua.set_app_data(s);
    let g = lua.globals();
    install_fs(lua)?;
    g.set(
        "docs",
        lua.create_function(|lua, topic: Option<String>| {
            let s = session(lua)?;
            let cfg = s.config();
            let disabled = cfg.disabled_builtins();
            Ok(crate::docs::docs_for(topic.as_deref(), &disabled))
        })?,
    )?;
    g.set(
        "set_title",
        lua.create_function(|lua, text: String| session(lua)?.set_name(&text).map_err(err))?,
    )?;
    g.set("session", lua.create_function(|lua, ()| lua_session(lua))?)?;
    install_tasks(lua)?;
    install_ui(lua)?;
    install_llm(lua)?;
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

fn install_fs(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    g.set("read", lua.create_function(read)?)?;
    g.set("write", lua.create_function(write)?)?;
    g.set("append", lua.create_function(append)?)?;
    g.set("delete", lua.create_function(delete)?)?;
    g.set("edit", lua.create_function(edit)?)?;
    g.set("grep", lua.create_function(grep)?)?;
    Ok(())
}

fn read(lua: &Lua, (path, o): (String, Option<Table>)) -> mlua::Result<String> {
    let cwd = match session(lua) {
        Ok(s) => {
            action(lua, &s, "reading", &path);
            s.cwd.clone()
        }
        Err(_) => tuse_lua::host(lua).cwd(),
    };
    let p = tuse_config::resolve_path(&path, &cwd);
    let lines = match &o {
        Some(t) => t.get::<Option<bool>>("lines")?.unwrap_or(false),
        None => false,
    };
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
    let r = tools::read(&p, lines, offset, limit).map_err(err)?;
    if let Some(url) = r.image {
        with_run(lua, |j| j.add_image(url));
    }
    Ok(r.text)
}

fn write(lua: &Lua, (path, content): (String, String)) -> mlua::Result<String> {
    let s = session(lua)?;
    action(lua, &s, "writing", &path);
    let p = tuse_config::resolve_path(&path, &s.cwd);
    let (msg, diff) = tools::write(&p, &content).map_err(err)?;
    with_run(lua, |j| j.add_diff(p.display().to_string(), diff));
    Ok(msg)
}

fn append(lua: &Lua, (path, text): (String, String)) -> mlua::Result<String> {
    let s = session(lua)?;
    action(lua, &s, "appending", &path);
    let p = tuse_config::resolve_path(&path, &s.cwd);
    let (msg, diff) = tools::append(&p, &text).map_err(err)?;
    with_run(lua, |j| j.add_diff(p.display().to_string(), diff));
    Ok(msg)
}

fn delete(lua: &Lua, path: String) -> mlua::Result<String> {
    let s = session(lua)?;
    action(lua, &s, "deleting", &path);
    let p = tuse_config::resolve_path(&path, &s.cwd);
    let (msg, diff) = tools::delete(&p).map_err(err)?;
    with_run(lua, |j| j.add_diff(p.display().to_string(), diff));
    Ok(msg)
}

fn edit(
    lua: &Lua,
    (path, old, new, o): (String, String, String, Option<Table>),
) -> mlua::Result<String> {
    let s = session(lua)?;
    action(lua, &s, "editing", &path);
    let p = tuse_config::resolve_path(&path, &s.cwd);
    let all = match o {
        Some(t) => t.get::<Option<bool>>("all")?.unwrap_or(false),
        None => false,
    };
    let (msg, diff) = tools::edit(&p, &old, &new, all).map_err(err)?;
    with_run(lua, |j| j.add_diff(p.display().to_string(), diff));
    Ok(msg)
}

fn grep(lua: &Lua, (pattern, o): (String, Option<Table>)) -> mlua::Result<String> {
    let s = session(lua)?;
    action(lua, &s, "searching", &pattern);
    let o = o.unwrap_or(lua.create_table()?);
    let n = |k: &str| -> mlua::Result<Option<usize>> {
        Ok(o.get::<Option<i64>>(k)?.map(|n| n.max(0) as usize))
    };
    let paths = grep_paths(&o)?;
    let opts = tools::GrepOpts {
        paths,
        glob: o.get("glob")?,
        ignore_case: o.get::<Option<bool>>("ignore_case")?.unwrap_or(false),
        literal: o.get::<Option<bool>>("literal")?.unwrap_or(false),
        context: n("context")?.unwrap_or(0),
        limit: n("limit")?,
    };
    let stopped = || tuse_lua::stopped(lua).is_some();
    tools::grep(&pattern, opts, &s.cwd, &stopped).map_err(err)
}

fn grep_paths(o: &Table) -> mlua::Result<Vec<String>> {
    match o.get::<Value>("path")? {
        Value::Nil => Ok(vec![]),
        Value::String(s) => Ok(vec![s.to_str()?.to_string()]),
        Value::Table(t) => {
            let mut out = vec![];
            for v in t.sequence_values::<Value>() {
                match v? {
                    Value::String(s) => out.push(s.to_str()?.to_string()),
                    _ => return Err(err("grep `path` list must contain strings".into())),
                }
            }
            Ok(out)
        }
        _ => Err(err("grep `path` must be a string or list of strings".into())),
    }
}

fn task(lua: &Lua, t: &TaskInfo, output: Option<String>) -> mlua::Result<Table> {
    let r = lua.create_table()?;
    r.set("id", t.id.as_str())?;
    r.set("name", t.name.as_str())?;
    r.set("status", t.status.as_str())?;
    r.set("started", t.started.as_str())?;
    r.set("ended", t.ended.as_str())?;
    r.set("code", t.code.as_str())?;
    r.set("output", output)?;
    Ok(r)
}

fn install_tasks(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    g.set(
        "task_get",
        lua.create_function(|lua, (id, lines): (String, Option<i64>)| {
            let s = session(lua)?;
            let t = s
                .tasks
                .get(&id)
                .ok_or_else(|| mlua::Error::runtime(format!("no task {id}")))?;
            let out = Tasks::tail(&t.log, lines.map_or(50, |n| n.max(1) as usize));
            task(lua, &t, Some(out))
        })?,
    )?;
    g.set(
        "task_stop",
        lua.create_function(|lua, id: String| {
            let s = session(lua)?;
            let t = s.stop_task(&id).map_err(mlua::Error::runtime)?;
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

fn install_ui(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    g.set(
        "show",
        lua.create_function(|lua, (name, text, pin): (String, String, Option<bool>)| {
            session(lua)?.ui_markdown(name, text, pin.unwrap_or(false));
            Ok(())
        })?,
    )?;
    let start_picker = lua.create_function(start_picker)?;
    let start_question = lua.create_function(start_question)?;
    g.set("pick", tuse_lua::yield_wait(lua, start_picker)?)?;
    g.set("ask", tuse_lua::yield_wait(lua, start_question)?)?;
    let start_reload = lua.create_function(start_reload)?;
    g.set("reload", tuse_lua::yield_wait(lua, start_reload)?)?;
    tuse_lua::wrap(lua, &g, &["show"])
}

fn start_picker(lua: &Lua, options: Table) -> mlua::Result<Value> {
    let mut list = vec![];
    for v in options.sequence_values::<Value>() {
        match v? {
            Value::String(s) => list.push(s.to_str()?.to_string()),
            _ => return Err(err("pick: options must be a list of strings".into())),
        }
    }
    if list.is_empty() {
        return Ok(Value::Nil);
    }
    enqueue(lua, UiPrompt::Picker { options: list })
}

fn start_question(lua: &Lua, text: String) -> mlua::Result<Value> {
    enqueue(lua, UiPrompt::Question { text })
}
fn start_reload(lua: &Lua, (): ()) -> mlua::Result<Value> {
    let s = session(lua)?;
    if tuse_lua::is_hook(lua) {
        s.reload_after_hook.store(true, Ordering::Release);
        return Ok(Value::Nil);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(s.reload().map(|_| serde_json::Value::Null));
    });
    let key = tuse_lua::add_wait(
        lua,
        ExtWait {
            rx,
            pause_timeout: true,
            on_cancel: None,
        },
    )?;
    Ok(Value::Integer(key as i64))
}

fn enqueue(lua: &Lua, prompt: UiPrompt) -> mlua::Result<Value> {
    let s = session(lua)?;
    let (id, rx) = s.ui_enqueue(prompt, tuse_lua::is_hook(lua)).map_err(err)?;
    let weak = Arc::downgrade(&s);
    let key = tuse_lua::add_wait(
        lua,
        ExtWait {
            rx,
            pause_timeout: true,
            on_cancel: Some(Box::new(move || {
                if let Some(s) = weak.upgrade() {
                    s.ui_cancel_request(id);
                }
            })),
        },
    )?;
    Ok(Value::Integer(key as i64))
}

fn install_llm(lua: &Lua) -> mlua::Result<()> {
    let start = lua.create_function(start)?;
    lua.globals().set("llm", tuse_lua::yield_wait(lua, start)?)
}

fn start(
    lua: &Lua,
    (prompt, model, effort): (String, Option<String>, Option<String>),
) -> mlua::Result<Value> {
    let s = session(lua)?;
    let (cur_model, cur_effort) = s.model();
    let model = model.filter(|m| !m.is_empty()).unwrap_or(cur_model);
    let effort = effort.filter(|e| !e.is_empty()).unwrap_or(cur_effort);
    if !tuse_config::EFFORTS.contains(&effort.as_str()) {
        return Err(err(format!(
            "llm: effort must be one of {}",
            tuse_config::EFFORTS.join(", ")
        )));
    }
    if model.is_empty() {
        return Err(err("llm: no model selected".into()));
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        let r = s.llm(&prompt, &model, &effort, &flag);
        let _ = tx.send(r.map(serde_json::Value::String));
    });
    let key = tuse_lua::add_wait(
        lua,
        ExtWait {
            rx,
            pause_timeout: false,
            on_cancel: Some(Box::new(move || {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed)
            })),
        },
    )?;
    Ok(Value::Integer(key as i64))
}

pub(crate) fn lua_session(lua: &Lua) -> mlua::Result<Value> {
    let pre = lua
        .app_data_ref::<crate::session::view::snapshot::LoadCtx>()
        .and_then(|ctx| {
            if ctx.loading {
                Some(
                    crate::session::view::snapshot::SessionSnapshot::pre_session(
                        ctx.mode,
                        ctx.configpath.clone(),
                        &ctx.cwd,
                        &tuse_config::tuse_home(),
                    ),
                )
            } else {
                None
            }
        });
    if let Some(snap) = pre {
        return snap.to_lua(lua);
    }
    if let Ok(s) = session(lua) {
        return s.snapshot(true).to_lua(lua);
    }
    let snap = lua
        .app_data_ref::<crate::session::view::snapshot::LoadCtx>()
        .map(|ctx| {
            crate::session::view::snapshot::SessionSnapshot::pre_session(
                ctx.mode,
                ctx.configpath.clone(),
                &ctx.cwd,
                &tuse_config::tuse_home(),
            )
        })
        .ok_or_else(|| mlua::Error::runtime("session closed"))?;
    snap.to_lua(lua)
}
