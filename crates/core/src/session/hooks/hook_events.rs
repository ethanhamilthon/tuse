use crate::session::Session;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tuse_lua::Done;

impl Session {
    pub(crate) fn simple_hook(self: &Arc<Self>, event: &'static str) {
        self.hook(event, json!({"event": event}));
    }

    pub(crate) fn lifecycle(self: &Arc<Self>, launch: bool, restored: bool) {
        self.simple_hook("config_load");
        if launch {
            self.simple_hook("process_start");
        }
        self.simple_hook(if restored {
            "session_resumed"
        } else {
            "session_created"
        });
    }

    pub(crate) fn agent_message_hook(self: &Arc<Self>, status: &'static str) {
        self.hook(
            "agent_message",
            json!({"event": "agent_message", "status": status}),
        )
    }

    pub(crate) fn lua_start_hook(
        self: &Arc<Self>,
        id: String,
        kind: &'static str,
        code: Option<String>,
        name: Option<String>,
    ) {
        if kind != "tool" {
            let job_code = code.clone().unwrap_or_default();
            self.jobs.lock().unwrap().insert(
                id.clone(),
                (kind, name.clone().unwrap_or_default(), job_code),
            );
        }
        self.hook(
            "lua_start",
            json!({"event": "lua_start", "id": id, "kind": kind, "code": code, "name": name}),
        );
    }

    pub(crate) fn lua_end_hook(
        self: &Arc<Self>,
        id: String,
        kind: &'static str,
        status: &'static str,
        elapsed: Duration,
        result: Option<String>,
        error: Option<String>,
    ) {
        if let Some((kind, name, code)) = self.jobs.lock().unwrap().remove(&id) {
            let r = crate::convo::record::Record::job(
                kind,
                name,
                code,
                if kind == "task" && status == "error" {
                    "failed"
                } else {
                    status
                }
                .into(),
                elapsed.as_millis() as u64,
                error.clone(),
            );
            let mut st = self.state.lock().unwrap();
            self.push(&mut st, r);
        }
        self.hook(
            "lua_end",
            json!({
                "event": "lua_end",
                "id": id,
                "kind": kind,
                "status": status,
                "duration_ms": elapsed.as_millis() as u64,
                "result": result,
                "error": error,
            }),
        );
    }

    pub(crate) fn lua_end_done(
        self: &Arc<Self>,
        id: &str,
        kind: &'static str,
        elapsed: Duration,
        d: &Done,
    ) {
        let (st, res, err) = if d.ok {
            ("success", Some(d.text.clone()), None)
        } else if d.stop == Some("cancelled") {
            ("cancelled", None, d.handed_off.then(|| d.text.clone()))
        } else {
            ("error", None, Some(d.text.clone()))
        };
        self.lua_end_hook(id.to_string(), kind, st, elapsed, res, err);
    }
}
