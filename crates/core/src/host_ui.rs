use crate::event::Event;
use crate::event::UiPrompt;
use crate::host::{err, session};
use crate::session::Session;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tuse_lua::mlua::{self, Lua, MultiValue, Table, Value};
use tuse_lua::{Done, ExtWait, Job, Out, Stop};

/// Installs `show_markdown`, `show_picker`, and `show_question`.
pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    g.set(
        "show_markdown",
        lua.create_function(|lua, (name, text, pin): (String, String, Option<bool>)| {
            session(lua)?.ui_markdown(name, text, pin.unwrap_or(false));
            Ok(())
        })?,
    )?;
    let start_picker = lua.create_function(start_picker)?;
    let start_question = lua.create_function(start_question)?;
    g.set("show_picker", tuse_lua::yield_wait(lua, start_picker)?)?;
    g.set("show_question", tuse_lua::yield_wait(lua, start_question)?)?;
    tuse_lua::wrap(lua, &g, &["show_markdown"])
}

fn start_picker(lua: &Lua, options: Table) -> mlua::Result<Value> {
    let mut list = vec![];
    for v in options.sequence_values::<Value>() {
        match v? {
            Value::String(s) => list.push(s.to_str()?.to_string()),
            _ => return Err(err("show_picker: options must be a list of strings".into())),
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

impl Session {
    /// Runs a configured custom slash command as a non-blocking Lua job.
    pub fn run_command(self: &Arc<Self>, name: &str) -> Result<(), String> {
        if !self.config().commands.iter().any(|c| c == name) {
            return Err(format!("unknown command /{name}"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut list = self.cmd_runs.lock().unwrap();
            list.retain(|c| !c.load(Ordering::Relaxed));
            list.push(cancel.clone());
        }
        let stop = Stop::new(cancel.clone(), None);
        let weak = Arc::downgrade(self);
        let cmd = name.to_string();
        let exec_id = self.next_lua_id();
        let started = Instant::now();
        self.lua_start_hook(exec_id.clone(), "command", None, Some(cmd.clone()));
        self.vm.post(move |lua| {
            let (s2, c2, flag, id2) = (weak.clone(), cmd.clone(), cancel.clone(), exec_id.clone());
            let done = move |_: &Lua, d: Done| {
                flag.store(true, Ordering::Relaxed);
                if let Some(s) = s2.upgrade() {
                    s.lua_end_done(&id2, "command", started.elapsed(), &d);
                    if !d.ok && d.stop != Some("cancelled") {
                        s.emit(Event::Error(format!(
                            "/{c2}: {}",
                            crate::host::error_line(&d.text)
                        )));
                    }
                }
            };
            let start = || -> mlua::Result<()> {
                let f = crate::config_lua::command_of(lua, &cmd)
                    .ok_or_else(|| mlua::Error::runtime("command is gone"))?;
                Job::new(
                    lua,
                    f,
                    MultiValue::new(),
                    stop,
                    Out::Buf(String::new()),
                    done,
                )?
                .start(lua);
                Ok(())
            };
            if let Err(e) = start() {
                cancel.store(true, Ordering::Relaxed);
                let msg = tuse_lua::message(&e);
                if let Some(s) = weak.upgrade() {
                    s.lua_end_hook(
                        exec_id,
                        "command",
                        "error",
                        started.elapsed(),
                        None,
                        Some(msg.clone()),
                    );
                    s.emit(Event::Error(format!("/{cmd}: {msg}")));
                }
            }
        });
        Ok(())
    }

    pub(crate) fn stop_commands(&self) {
        for c in self.cmd_runs.lock().unwrap().drain(..) {
            c.store(true, Ordering::Relaxed);
        }
    }
}
