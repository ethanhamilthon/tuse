//! The methods a client calls; each does what the TUI does with the same session call.
use crate::proto::{Fail, Reply, str_list, str_param};
use serde_json::{Value, json};
use std::sync::Arc;
use tuse_core::{InputId, Session, UserInput};

pub const PROTOCOL: u64 = 1;

pub const METHODS: &[&str] = &[
    "initialize",
    "turn.submit",
    "turn.withdraw",
    "turn.cancel",
    "session.info",
    "session.records",
    "session.trace",
    "session.stats",
    "session.list",
    "model.get",
    "model.set",
    "model.list",
    "config.get",
    "config.reload",
    "config.set",
    "tasks.list",
    "tasks.get",
    "tasks.stop",
    "shutdown",
];

fn info(s: &Session) -> Value {
    let turn = s.turn_info().map(
        |(elapsed, status)| json!({"elapsedMs": elapsed.as_millis() as u64, "status": status}),
    );
    json!({
        "id": s.id,
        "title": s.title(),
        "cwd": s.cwd.display().to_string(),
        "busy": s.busy(),
        "turn": turn,
    })
}

pub fn initialize(s: &Session) -> Value {
    let (model, effort) = s.model();
    json!({
        "protocolVersion": PROTOCOL,
        "tuse": env!("CARGO_PKG_VERSION"),
        "session": info(s),
        "model": model,
        "effort": effort,
        "capabilities": {"methods": METHODS},
    })
}

fn submit(s: &Arc<Session>, p: &Value) -> Reply {
    let text = str_param(p, "text")?;
    let (urls, paths): (Vec<String>, Vec<String>) = str_list(p, "images")?
        .into_iter()
        .partition(|f| f.starts_with("data:"));
    let refs = tuse_core::Refs {
        images: (1..).zip(paths.into_iter().map(Into::into)).collect(),
        files: str_list(p, "files")?.into_iter().map(Into::into).collect(),
        cwd: s.cwd.clone(),
        strict: true,
        ..Default::default()
    };
    let mut input: UserInput = tuse_core::build_input(&text, refs).map_err(Fail::session)?;
    input.images.extend(urls);
    if input.text.trim().is_empty() && input.context.is_empty() && input.images.is_empty() {
        return Err(Fail::session("no input"));
    }
    let (id, queued) = s.submit(input);
    Ok(json!({"inputId": id.number(), "queued": queued}))
}

fn stats(s: &Session) -> Value {
    let st = s.stats();
    json!({
        "input": st.totals.input,
        "output": st.totals.output,
        "cached": st.totals.cached,
        "cacheWrite": st.totals.cache_write,
        "cost": st.priced.then_some(st.cost),
        "requests": st.requests,
        "context": st.context,
        "turns": st.turns,
        "calls": st.calls,
    })
}

/// Runs one method. `shutdown` is handled by the server.
pub fn call(s: &Arc<Session>, method: &str, p: &Value) -> Reply {
    match method {
        "turn.submit" => submit(s, p),
        "turn.withdraw" => {
            let n = p["inputId"]
                .as_u64()
                .ok_or_else(|| Fail::params("`inputId` must be a number"))?;
            Ok(json!(s.withdraw_input(InputId::from_number(n))))
        }
        "turn.cancel" => {
            s.cancel();
            Ok(Value::Null)
        }
        "session.info" => Ok(info(s)),
        "session.records" => s
            .with_records(|r| serde_json::to_value(r))
            .map_err(|e| Fail::session(e.to_string())),
        "session.trace" => Ok(s
            .trace()
            .into_iter()
            .map(|t| json!({"role": t.role, "summary": t.summary, "text": t.text}))
            .collect()),
        "session.stats" => Ok(stats(s)),
        "session.list" => {
            let cwd = p["cwd"].as_str().map_or_else(|| s.cwd.clone(), Into::into);
            serde_json::to_value(tuse_core::list_sessions(&cwd))
                .map_err(|e| Fail::session(e.to_string()))
        }
        "model.get" => {
            let (model, effort) = s.model();
            Ok(json!({"model": model, "effort": effort}))
        }
        "model.set" => {
            let model = str_param(p, "model")?;
            let effort = p["effort"].as_str().unwrap_or("default");
            if !tuse_config::EFFORTS.contains(&effort) {
                return Err(Fail::params(format!(
                    "`effort` must be one of {}",
                    tuse_config::EFFORTS.join(", ")
                )));
            }
            s.set_model(&model, effort);
            Ok(Value::Null)
        }
        "model.list" => {
            s.fetch_models();
            Ok(json!(s.available_models()))
        }
        "config.get" => {
            serde_json::to_value(&*s.config()).map_err(|e| Fail::session(e.to_string()))
        }
        "config.reload" => s.reload().map(|_| Value::Null).map_err(Fail::session),
        "config.set" => {
            let key = str_param(p, "key")?;
            let value = str_param(p, "value")?;
            if !matches!(key.as_str(), "model" | "effort") {
                return Err(Fail::params("`key` must be `model` or `effort`"));
            }
            s.save_choice(&key, &value)
                .map(|_| Value::Null)
                .map_err(Fail::session)
        }
        "tasks.list" => {
            serde_json::to_value(s.tasks.list()).map_err(|e| Fail::session(e.to_string()))
        }
        "tasks.get" => {
            let id = str_param(p, "id")?;
            let t = s
                .tasks
                .get(&id)
                .ok_or_else(|| Fail::session(format!("no task {id}")))?;
            let lines = p["lines"].as_u64().unwrap_or(50) as usize;
            let mut v = serde_json::to_value(&t).map_err(|e| Fail::session(e.to_string()))?;
            v["output"] = json!(tuse_core::tail(&t.log, lines));
            Ok(v)
        }
        "tasks.stop" => {
            let id = str_param(p, "id")?;
            let t = s.tasks.stop(&id).map_err(Fail::session)?;
            serde_json::to_value(t).map_err(|e| Fail::session(e.to_string()))
        }
        _ => Err(Fail(
            crate::proto::NO_METHOD,
            format!("unknown method `{method}`"),
        )),
    }
}
