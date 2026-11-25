use crate::convo::record::Record;
use crate::session::Session;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tuse_api::{ApiType, Client, Message, Request};
use tuse_config::split_model;
use tuse_lua::mlua::{self, Lua, MultiValue};
use tuse_lua::{Done, Job, Out, Stop};

impl Session {
    pub fn run_command(self: &Arc<Self>, name: &str) -> Result<(), String> {
        if !self.config().commands.iter().any(|c| c == name) {
            let error = format!("unknown command /{name}");
            self.error(error.clone());
            return Err(error);
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
        self.vm.post(move |lua| {
            if let Some(s) = weak.upgrade() {
                let code = crate::scripting::registry::command_of(lua, &cmd)
                    .map(|f| tuse_lua::source_of(&f, None));
                s.lua_start_hook(exec_id.clone(), "command", code, Some(cmd.clone()));
            }
            let (s2, c2, flag, id2) = (weak.clone(), cmd.clone(), cancel.clone(), exec_id.clone());
            let done = move |_: &Lua, d: Done| {
                flag.store(true, Ordering::Relaxed);
                if let Some(s) = s2.upgrade() {
                    s.lua_end_done(&id2, "command", started.elapsed(), &d);
                    if !d.ok && d.stop != Some("cancelled") {
                        s.error(format!("/{c2}: {}", crate::text::error_line(&d.text)));
                    }
                }
            };
            let start = || -> mlua::Result<()> {
                let f = crate::scripting::registry::command_of(lua, &cmd)
                    .ok_or_else(|| mlua::Error::runtime("command is gone"))?;
                let job = Job::new(
                    lua,
                    f,
                    MultiValue::new(),
                    stop,
                    Out::Buf(String::new()),
                    done,
                )?;
                tuse_lua::start(lua, job);
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
                    s.error(format!("/{cmd}: {msg}"));
                }
            }
        });
        Ok(())
    }
}

impl Session {
    pub(crate) fn prompt_body(&self, name: &str, arg: Option<&str>) -> Result<String, String> {
        let cancel = Arc::new(AtomicBool::new(false));
        self.cmd_runs.lock().unwrap().push(cancel.clone());
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let stop = Stop::new(cancel, Some(deadline));
        let (tx, rx) = std::sync::mpsc::channel();
        let (name, arg) = (name.to_string(), arg.map(str::to_string));
        self.vm.post(move |lua| {
            let start = || -> mlua::Result<()> {
                let f = crate::scripting::registry::prompt_of(lua, &name)
                    .ok_or_else(|| mlua::Error::runtime("prompt is gone"))?;
                let args = match &arg {
                    Some(a) => MultiValue::from_iter([mlua::Value::String(lua.create_string(a)?)]),
                    None => MultiValue::new(),
                };
                let tx2 = tx.clone();
                let job = Job::new(lua, f, args, stop, Out::Buf(String::new()), move |_, d| {
                    let _ = tx2.send(d);
                })?
                .returning();
                tuse_lua::start(lua, job);
                Ok(())
            };
            if let Err(e) = start() {
                let _ = tx.send(Done::failed(tuse_lua::message(&e)));
            }
        });
        let d = rx
            .recv()
            .map_err(|_| "the lua thread is gone".to_string())?;
        if !d.ok {
            return Err(format!("prompt: {}", crate::text::error_line(&d.text)));
        }
        Ok(d.text)
    }

    pub(crate) fn llm(
        &self,
        prompt: &str,
        model: &str,
        effort: &str,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let config = self.config();
        let (provider, name) = split_model(model);
        let conn = config
            .connect(provider)
            .ok_or_else(|| format!("llm: no provider `{provider}` for model `{model}`"))?;
        let req = Request {
            provider: provider.to_string(),
            model: name.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                text: prompt.to_string(),
                images: vec![],
            }],
            tools: vec![],
            effort: (effort != "default").then(|| effort.to_string()),
        };
        let client = Client::with_agent(
            self.http.clone(),
            &conn.base_url,
            &conn.api_key,
            ApiType::from_stored(conn.api_type()),
        );
        let reply = client
            .complete(
                &req,
                tuse_api::RETRY_DELAYS.len(),
                cancel,
                &mut |_| {},
                &mut |_| {},
            )
            .map_err(|e| format!("llm: {e}"))?;
        if let Some(u) = reply.usage {
            let mut st = self.state.lock().unwrap();
            self.record(
                &mut st,
                &[Record::Usage {
                    model: model.to_string(),
                    usage: u,
                    llm: true,
                }],
                [crate::Event::Usage],
            );
        }
        Ok(reply.text)
    }
}
