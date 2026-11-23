use crate::event::Event;
use crate::id;
use crate::record::{FileDiff, Record};
use crate::session::Session;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tuse_api::ToolCall;

/// The only tool.
pub const TOOL: &str = "lua";

/// Seconds a `lua` call gets before it is stopped.
pub const TIMEOUT_SECS: u64 = 120;

/// The result of one call.
struct Outcome {
    text: String,
    /// `0` success, `1` an error or a timeout, `None` a cancel.
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
    #[allow(dead_code)] // used again by the action status
    pub(crate) fn set_status(&self, text: String) {
        let mut st = self.state.lock().unwrap();
        if st.status == text {
            return;
        }
        st.status = text.clone();
        drop(st);
        self.emit(Event::Status(text));
    }

    /// Runs one tool call and records its result. False when the turn went stale.
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
            Outcome::err(format!(
                "unknown tool `{}`; the only tool is `{TOOL}`: call functions such as `read` or `sh` inside its `code`",
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

    /// Runs `code` as a job of the session's VM.
    fn lua_call(self: &Arc<Self>, args: &serde_json::Value, cancel: &Arc<AtomicBool>) -> Outcome {
        let Some(code) = args["code"].as_str() else {
            return Outcome::err("missing argument `code` (a string)");
        };
        let stop = tuse_lua::Stop::new(
            cancel.clone(),
            Some(Instant::now() + Duration::from_secs(TIMEOUT_SECS)),
        );
        let d = self.vm.run(code, "lua", stop);
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
