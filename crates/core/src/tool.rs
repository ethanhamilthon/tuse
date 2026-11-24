use crate::event::Event;
use crate::id;
use crate::record::{FileDiff, Record};
use crate::session::Session;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tuse_api::ToolCall;

pub const TOOL: &str = "lua";

pub const TIMEOUT_SECS: u64 = 120;

struct Outcome {
    text: String,
    code: Option<i32>,
    images: Vec<String>,
    diffs: Vec<FileDiff>,
}

impl Outcome {
    fn err(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            code: Some(1),
            images: vec![],
            diffs: vec![],
        }
    }
}

impl Session {
    pub(crate) fn set_status(&self, text: String) {
        let mut st = self.state.lock().unwrap();
        if st.status == text {
            return;
        }
        st.status = text.clone();
        drop(st);
        self.emit(Event::Status(text));
    }

    pub(crate) fn tool(
        self: &Arc<Self>,
        generation: u64,
        cancel: &Arc<AtomicBool>,
        call: &ToolCall,
    ) -> bool {
        let parsed = serde_json::from_str::<serde_json::Value>(&call.arguments);
        let args = parsed.as_ref().cloned().unwrap_or_default();
        let input = args["code"]
            .as_str()
            .map_or_else(|| call.arguments.clone(), String::from);
        self.emit(Event::ToolStart {
            timeout: TIMEOUT_SECS,
            id: call.id.clone(),
            tool: call.name.clone(),
            input: input.clone(),
        });
        let out = if parsed.is_err() {
            Outcome::err(format!(
                "the arguments are not valid JSON: {}",
                call.arguments
            ))
        } else if call.name == TOOL {
            self.lua_call(&args, cancel)
        } else {
            let cfg = self.config();
            let hint = match (!cfg.is_disabled("read"), !cfg.is_disabled("sh")) {
                (true, true) => "call functions such as `read` or `sh` inside its `code`",
                (true, false) => "call functions such as `read` inside its `code`",
                (false, true) => "call functions such as `sh` inside its `code`",
                (false, false) => "call functions inside its `code`",
            };
            Outcome::err(format!(
                "unknown tool `{}`; the only tool is `{TOOL}`: {hint}",
                call.name
            ))
        };
        let text = out.text;
        let Some(mut st) = self.lock_live(generation) else {
            return false;
        };
        st.calls += 1;
        let r = Record::Tool {
            id: call.id.clone(),
            tool: call.name.clone(),
            input: input.clone(),
            output: text.clone(),
            code: out.code,
            images: out.images,
            diffs: out.diffs.clone(),
            at: id::now(),
        };
        self.push(&mut st, r);
        drop(st);
        self.emit(Event::ToolEnd {
            id: call.id.clone(),
            output: text,
            code: out.code,
            diffs: out.diffs,
        });
        true
    }

    fn lua_call(self: &Arc<Self>, args: &serde_json::Value, cancel: &Arc<AtomicBool>) -> Outcome {
        let Some(code) = args["code"].as_str() else {
            return Outcome::err("missing argument `code` (a string)");
        };
        let stop = tuse_lua::Stop::new(
            cancel.clone(),
            Some(Instant::now() + Duration::from_secs(TIMEOUT_SECS)),
        );
        let exec_id = self.next_lua_id();
        let started = Instant::now();
        self.lua_start_hook(exec_id.clone(), "tool", Some(code.to_string()), None);
        let d = self.vm.run(code, "lua", stop);
        self.lua_end_done(&exec_id, "tool", started.elapsed(), &d);
        Outcome {
            code: match (d.ok, d.stop) {
                (true, _) => Some(0),
                (false, Some("cancelled")) => None,
                _ => Some(1),
            },
            text: d.text,
            images: d.images,
            diffs: d
                .diffs
                .into_iter()
                .map(|(path, diff)| FileDiff { path, diff })
                .collect(),
        }
    }
}
