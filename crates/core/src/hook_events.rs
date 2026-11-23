//! Typed helpers that emit each hook event with its event-only payload.

use crate::session::Session;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tuse_api::ToolCall;
use tuse_config::split_model;
use tuse_lua::Done;

pub(crate) fn compact(mut v: Value) -> Value {
    if let Value::Object(m) = &mut v {
        m.retain(|_, val| !val.is_null());
    }
    v
}

fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

impl Session {
    pub(crate) fn start_hook(self: &Arc<Self>, reason: &'static str, is_new: bool, restored: bool) {
        self.hook(
            "start",
            json!({
                "event": "start",
                "reason": reason,
                "is_new": is_new,
                "restored": restored,
            }),
        );
    }

    pub(crate) fn user_message_hook(self: &Arc<Self>, text: String, is_first: bool) {
        self.hook(
            "user_message",
            json!({"event": "user_message", "text": text, "is_first": is_first}),
        );
    }

    pub(crate) fn agent_message_hook(
        self: &Arc<Self>,
        text: String,
        status: &'static str,
        calls: &[ToolCall],
    ) {
        let tc = (!calls.is_empty()).then(|| {
            calls
                .iter()
                .map(|c| json!({"id": c.id, "name": c.name, "arguments": c.arguments}))
                .collect::<Vec<_>>()
        });
        self.hook(
            "agent_message",
            json!({"event": "agent_message", "text": text, "status": status, "tool_calls": tc}),
        );
    }

    pub(crate) fn model_change_hook(self: &Arc<Self>, pm: &str, pe: &str, nm: &str, ne: &str) {
        let side = |m: &str, e: &str| {
            let (p, _) = split_model(m);
            compact(json!({"model": non_empty(m), "provider": non_empty(p), "effort": e}))
        };
        self.hook(
            "model_change",
            json!({
                "event": "model_change",
                "previous": side(pm, pe),
                "new": side(nm, ne),
            }),
        );
    }

    pub(crate) fn name_change_hook(self: &Arc<Self>, previous: Option<String>, new: String) {
        self.hook(
            "name_change",
            json!({"event": "name_change", "previous": previous, "new": new}),
        );
    }

    pub(crate) fn lua_start_hook(
        self: &Arc<Self>,
        id: String,
        kind: &'static str,
        code: Option<String>,
        name: Option<String>,
    ) {
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
            ("cancelled", None, None)
        } else {
            ("error", None, Some(d.text.clone()))
        };
        self.lua_end_hook(id.to_string(), kind, st, elapsed, res, err);
    }
}
