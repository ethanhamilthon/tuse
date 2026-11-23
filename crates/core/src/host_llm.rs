use crate::host::{err, session};
use crate::record::Record;
use crate::session::Session;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tuse_api::{Client, Request};
use tuse_config::split_model;
use tuse_lua::ExtWait;
use tuse_lua::mlua::{self, Lua, Value};

/// Installs `llm(prompt, model, effort)`.
pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
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

impl Session {
    /// One request without tools or history; its usage is saved as `llm` usage.
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
            model: name.to_string(),
            messages: vec![serde_json::json!({"role": "user", "content": prompt})],
            tools: vec![],
            effort: (effort != "default").then(|| effort.to_string()),
        };
        let client = Client::with_agent(self.http.clone(), &conn.base_url, &conn.api_key);
        let reply = client
            .complete(
                &req,
                config.settings.retries,
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
