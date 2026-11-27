use crate::event::UiPrompt;
use crate::session::Session;
use crate::session::bus::Signal;
use crate::storage::tasks::{TaskInfo, Tasks};
use crate::tools;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;
use tuse_lua::mlua::{self, Lua, LuaSerdeExt, Table, Value};
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
        if from_lua_hook {
            s.task_lua
                .lock()
                .unwrap()
                .insert(t.id.clone(), (String::new(), Instant::now()));
        } else {
            self.task_track(&t.id, name);
        }
        s.emit(Signal::Tasks);
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
        let tracked = s.task_lua.lock().unwrap().remove(id);
        if let Some((exec_id, started)) = tracked {
            if exec_id.is_empty() {
                if let Some(t) = s.tasks.get(id) {
                    let status = if d.ok {
                        "success"
                    } else if d.stop == Some("cancelled") {
                        "cancelled"
                    } else {
                        "failed"
                    };
                    let record = crate::Record::job(
                        "task",
                        t.name,
                        t.code,
                        status.into(),
                        started.elapsed().as_millis() as u64,
                        (!d.ok).then(|| crate::text::error_line(&d.text)),
                    );
                    s.push(&mut s.state.lock().unwrap(), record);
                }
            } else {
                s.lua_end_done(&exec_id, "task", started.elapsed(), &d);
            }
        }
        let ended = s.tasks.end(id, d.ok);
        s.emit(Signal::Tasks);
        if let Some(t) = ended {
            let last = if d.ok {
                crate::text::last_line(&d.text).map(str::to_string)
            } else {
                Some(crate::text::error_line(&d.text))
            };
            let what = if d.ok { "finished" } else { "failed" };
            let tail = last.map_or_else(String::new, |l| format!(" Last line: {l}."));
            let out_hint = if s.config().is_disabled("task_get") {
                String::new()
            } else {
                format!(" Output: `task_get(\"{}\").output`.", t.id)
            };
            let msg = format!(
                "<task>Task {} ({}) {what}.{tail}{out_hint}</task>",
                t.id, t.name
            );
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
    g.set("read", lua.create_function(read)?)?;
    g.set(
        "write",
        lua.create_function(|l, (p, c): (String, String)| {
            mutate(l, "writing", &p, |f| tools::write(f, &c))
        })?,
    )?;
    g.set(
        "append",
        lua.create_function(|l, (p, t): (String, String)| {
            mutate(l, "appending", &p, |f| tools::append(f, &t))
        })?,
    )?;
    g.set(
        "delete",
        lua.create_function(|l, p: String| mutate(l, "deleting", &p, tools::delete))?,
    )?;
    g.set(
        "edit",
        lua.create_function(
            |l, (p, old, new, o): (String, String, String, Option<Table>)| {
                let all = o
                    .map(|t| t.get::<Option<bool>>("all"))
                    .transpose()?
                    .flatten()
                    .unwrap_or(false);
                mutate(l, "editing", &p, |f| tools::edit(f, &old, &new, all))
            },
        )?,
    )?;
    g.set("grep", lua.create_function(grep)?)?;
    g.set(
        "docs",
        lua.create_function(|lua, topic: Option<String>| {
            let disabled = session(lua)?.config().disabled_builtins();
            Ok(crate::docs::docs_for(topic.as_deref(), &disabled))
        })?,
    )?;
    g.set("set_session", lua.create_function(set_session)?)?;
    g.set("session", lua.create_function(|lua, ()| lua_session(lua))?)?;
    g.set("fork", lua.create_function(fork)?)?;
    g.set("resume", lua.create_function(resume)?)?;
    install_tasks(lua)?;
    install_ui(lua)?;
    let llm_start = lua.create_function(start)?;
    g.set("llm", tuse_lua::yield_wait(lua, llm_start)?)?;
    tuse_lua::wrap(
        lua,
        &g,
        &[
            "read",
            "write",
            "append",
            "delete",
            "edit",
            "grep",
            "docs",
            "set_session",
            "session",
            "fork",
            "resume",
            "task_get",
            "task_stop",
            "task_list",
        ],
    )
}

fn mutate(
    lua: &Lua,
    verb: &str,
    path: &str,
    op: impl FnOnce(&std::path::Path) -> Result<(String, String), String>,
) -> mlua::Result<String> {
    let s = session(lua)?;
    action(lua, &s, verb, path);
    let p = tuse_config::resolve_path(path, &s.cwd);
    let (msg, diff) = op(&p).map_err(err)?;
    with_run(lua, |j| j.add_diff(p.display().to_string(), diff));
    Ok(msg)
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
    let lines = o
        .as_ref()
        .map(|t| t.get::<Option<bool>>("lines"))
        .transpose()?
        .flatten()
        .unwrap_or(false);
    let opt = |k: &str| -> mlua::Result<Option<usize>> {
        Ok(o.as_ref()
            .map(|t| t.get::<Option<i64>>(k))
            .transpose()?
            .flatten()
            .map(|n| n.max(1) as usize))
    };
    let r = tools::read(
        &p,
        lines,
        opt("offset")?.unwrap_or(1),
        opt("limit")?.unwrap_or(tools::DEFAULT_LIMIT),
    )
    .map_err(err)?;
    if let Some(url) = r.image {
        with_run(lua, |j| j.add_image(url));
    }
    Ok(r.text)
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
    r.set("log", t.log.as_str())?;
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

fn start_picker(lua: &Lua, (options, title): (Table, Option<String>)) -> mlua::Result<Value> {
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
    enqueue(
        lua,
        UiPrompt::Picker {
            options: list,
            title: title.filter(|s| !s.trim().is_empty()),
        },
    )
}

fn start_question(lua: &Lua, text: String) -> mlua::Result<Value> {
    enqueue(lua, UiPrompt::Question { text })
}
fn start_reload(lua: &Lua, ns: Value) -> mlua::Result<Value> {
    let ns = match ns {
        Value::Nil => None,
        Value::String(ns) if !ns.as_bytes().is_empty() => Some(ns.to_str()?.to_owned()),
        _ => return Err(mlua::Error::runtime("ns must be a non-empty string")),
    };
    let s = session(lua)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let start = move || {
        std::thread::spawn(move || {
            let _ = tx.send(s.reload_namespace(ns).map(|_| serde_json::Value::Null));
        });
    };
    if tuse_lua::is_hook(lua) {
        with_run(lua, |job| job.after_done(start));
        return Ok(Value::Nil);
    }
    start();
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

fn start(
    lua: &Lua,
    (prompt, model, effort): (String, Option<String>, Option<String>),
) -> mlua::Result<Value> {
    let s = session(lua)?;
    let config = s.config();
    let (cur_model, cur_effort) = s.model();
    let explicit_model = model.filter(|m| !m.is_empty());
    let switched = explicit_model.as_deref().is_some_and(|m| m != cur_model);
    let model = explicit_model.unwrap_or(cur_model);
    if model.is_empty() {
        return Err(err("llm: no model selected".into()));
    }
    let effs = config.model_efforts(&model);
    let effort = match effort.filter(|e| !e.is_empty()) {
        Some(e) => {
            config
                .check_effort(&model, &e)
                .map_err(|m| err(format!("llm: {m}")))?;
            e
        }
        None if switched => {
            if effs.is_empty() {
                String::new()
            } else if effs.iter().any(|e| e == "low") {
                "low".into()
            } else {
                return Err(err(format!(
                    "llm: model `{model}` does not support `low` effort (supported: {})",
                    effs.join(", ")
                )));
            }
        }
        None => {
            config
                .check_effort(&model, &cur_effort)
                .map_err(|m| err(format!("llm: {m}")))?;
            cur_effort
        }
    };
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
    let loading = lua
        .app_data_ref::<crate::session::view::snapshot::LoadCtx>()
        .is_some_and(|c| c.loading);
    if !loading && let Ok(s) = session(lua) {
        return s.snapshot(true).to_lua(lua);
    }
    let ctx = lua
        .app_data_ref::<crate::session::view::snapshot::LoadCtx>()
        .ok_or_else(|| mlua::Error::runtime("session closed"))?;
    crate::session::view::snapshot::SessionSnapshot::pre_session(
        ctx.mode,
        ctx.config_path.clone(),
        &ctx.cwd,
        &tuse_config::tuse_home(),
    )
    .to_lua(lua)
}
fn set_session(lua: &Lua, table: Table) -> mlua::Result<()> {
    let mut update = crate::session::view::history::Update::default();
    for pair in table.pairs::<Value, Value>() {
        let (key, value) = pair?;
        let Value::String(key) = key else {
            return Err(mlua::Error::runtime("set_session keys must be strings"));
        };
        let key = key.to_str()?;
        match &*key {
            "history" => {
                let Value::Table(history) = value else {
                    return Err(mlua::Error::runtime("history must be a list"));
                };
                let list = dense(&history, "history")?
                    .into_iter()
                    .enumerate()
                    .map(|(i, value)| {
                        let Value::Table(r) = value else {
                            return Err(mlua::Error::runtime(format!(
                                "history[{}] must be a record",
                                i + 1
                            )));
                        };
                        let mut fields = serde_json::Map::new();
                        for pair in r.pairs::<Value, Value>() {
                            let (key, value) = pair?;
                            let Value::String(key) = key else {
                                return Err(mlua::Error::runtime(format!(
                                    "history[{}] field names must be strings",
                                    i + 1
                                )));
                            };
                            let key = key.to_str()?.to_string();
                            let value = match key.as_str() {
                                "summary" => continue,
                                "images" | "tool_calls" | "diffs" => {
                                    let Value::Table(t) = value else {
                                        return Err(mlua::Error::runtime(format!(
                                            "`{key}` must be a list"
                                        )));
                                    };
                                    serde_json::Value::Array(
                                        dense(&t, &key)?
                                            .into_iter()
                                            .map(|v| lua.from_value(v))
                                            .collect::<mlua::Result<_>>()?,
                                    )
                                }
                                _ => lua.from_value(value)?,
                            };
                            fields.insert(key, value);
                        }
                        Ok(fields)
                    })
                    .collect::<mlua::Result<_>>()?;
                update.history = Some(list);
            }
            "title" | "model" | "effort" => {
                let Value::String(value) = value else {
                    return Err(mlua::Error::runtime(format!(
                        "set_session `{key}` must be a string"
                    )));
                };
                let value = Some(value.to_str()?.to_string());
                match &*key {
                    "title" => update.title = value,
                    "model" => update.model = value,
                    _ => update.effort = value,
                }
            }
            _ => {
                return Err(mlua::Error::runtime(format!(
                    "unknown set_session key `{key}`"
                )));
            }
        }
    }
    session(lua)?.set_session(update).map_err(err)
}

fn fork(lua: &Lua, position: i64) -> mlua::Result<()> {
    let index = usize::try_from(position)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .ok_or_else(|| mlua::Error::runtime("fork position must be >= 1"))?;
    session(lua)?
        .request_switch(crate::event::Switch::Fork(index))
        .map_err(err)
}

fn resume(lua: &Lua, id: String) -> mlua::Result<()> {
    session(lua)?
        .request_switch(crate::event::Switch::Resume(id))
        .map_err(err)
}

fn dense(t: &Table, name: &str) -> mlua::Result<Vec<Value>> {
    let len = t.raw_len();
    let fail = || mlua::Error::runtime(format!("`{name}` must be a dense list"));
    for pair in t.pairs::<Value, Value>() {
        let (key, _) = pair?;
        if !matches!(key, Value::Integer(i) if i > 0 && i as usize <= len) {
            return Err(fail());
        }
    }
    (1..=len)
        .map(|i| {
            let v = t.raw_get::<Value>(i)?;
            if v.is_nil() { Err(fail()) } else { Ok(v) }
        })
        .collect()
}
