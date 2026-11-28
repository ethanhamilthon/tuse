use crate::convo::record::Record;
use crate::session::Session;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tuse_api::{ApiType, Client, Message, Request};
use tuse_config::split_model;
use tuse_lua::mlua::{Lua, MultiValue};
use tuse_lua::{Done, Job, Out, Stop};

impl Session {
    pub fn run_command(self: &Arc<Self>, name: &str, argv: &[String]) -> Result<(), String> {
        let checked = if !self.config().commands.contains(name) {
            Err(format!("unknown command /{name}"))
        } else if !argv.is_empty() {
            Err(format!("/{name} takes no words"))
        } else {
            Ok(())
        };
        if let Err(error) = checked {
            self.error(error.clone());
            return Err(error);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut list = self.cmd_runs.lock().unwrap();
            list.retain(|c| !c.load(Ordering::Relaxed));
            list.push(cancel.clone());
        }
        let weak = Arc::downgrade(self);
        let cmd = name.to_string();
        self.vm.post(move |lua| {
            let Some(s) = weak.upgrade() else { return };
            let Some(f) = crate::scripting::registry::command_of(lua, &cmd) else {
                cancel.store(true, Ordering::Relaxed);
                s.error(format!("/{cmd}: command is gone"));
                return;
            };
            let code = tuse_lua::source_of(&f, None);
            let started = Instant::now();
            let stop = Stop::new(cancel.clone(), None);
            let flag = cancel.clone();
            let name = cmd.clone();
            let done = move |_: &Lua, d: Done| {
                flag.store(true, Ordering::Relaxed);
                if let Some(s) = weak.upgrade() {
                    let status = if d.ok {
                        "success"
                    } else if d.stop == Some("cancelled") {
                        "cancelled"
                    } else {
                        "error"
                    };
                    let error = (status == "error").then(|| crate::text::error_line(&d.text));
                    let record = Record::job(
                        "command",
                        name.clone(),
                        code,
                        status.into(),
                        started.elapsed().as_millis() as u64,
                        error.clone(),
                    );
                    s.push(&mut s.state.lock().unwrap(), record);
                    if let Some(error) = error {
                        s.error(format!("/{name}: {error}"));
                    }
                }
            };
            match Job::new(
                lua,
                f,
                MultiValue::new(),
                stop,
                Out::Buf(String::new()),
                done,
            ) {
                Ok(job) => tuse_lua::start(lua, job),
                Err(e) => {
                    cancel.store(true, Ordering::Relaxed);
                    s.error(format!("/{cmd}: {}", tuse_lua::message(&e)));
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
        let (provider, name) = split_model(model);
        let conn = crate::storage::Providers::resolve_by_name(provider)
            .map_err(|e| format!("llm: {e}"))?
            .ok_or_else(|| {
                format!("llm: provider `{provider}` for model `{model}` is not connected")
            })?;
        let req = Request {
            provider: provider.to_string(),
            model: name.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                text: prompt.to_string(),
                images: vec![],
            }],
            tools: vec![],
            effort: (!effort.is_empty()).then(|| effort.to_string()),
        };
        let client = Client::with_agent(
            self.http.clone(),
            &conn.base_url,
            &conn.api_key,
            ApiType::from_stored(conn.api_type()),
        )
        .with_headers(conn.headers.clone());
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
                &[Record::usage(model.to_string(), u, true)],
                [crate::Event::Usage],
            );
        }
        Ok(reply.text)
    }
}
