use crate::convo::record::Record;
use crate::event::Event;
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
                            crate::text::error_line(&d.text)
                        )));
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
                    s.emit(Event::Error(format!("/{cmd}: {msg}")));
                }
            }
        });
        Ok(())
    }
}

impl Session {
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
            self.push(
                &mut st,
                Record::Usage {
                    model: model.to_string(),
                    usage: u,
                    llm: true,
                },
            );
            drop(st);
            self.emit(crate::Event::Usage);
        }
        Ok(reply.text)
    }
}
