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
        config: &tuse_config::Config,
    ) -> bool {
        let parsed = serde_json::from_str::<serde_json::Value>(&call.arguments);
        let args = parsed.as_ref().cloned().unwrap_or_default();
        let input = args["code"]
            .as_str()
            .map_or_else(|| call.arguments.clone(), String::from);
        let title = args["title"].as_str().unwrap_or("").trim().to_string();
        if !title.is_empty() {
            self.set_status(title.clone());
        }
        let secs = timeout_of(&args, config);
        self.emit(Event::ToolStart {
            timeout: secs,
            id: call.id.clone(),
            tool: call.name.clone(),
            input: input.clone(),
            title: title.clone(),
        });
        let out = if parsed.is_err() {
            Outcome::err(format!(
                "the arguments are not valid JSON: {}",
                call.arguments
            ))
        } else if call.name == TOOL {
            self.lua_call(&args, cancel, secs)
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
            title: title.clone(),
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
    fn lua_call(
        self: &Arc<Self>,
        args: &serde_json::Value,
        cancel: &Arc<AtomicBool>,
        secs: u64,
    ) -> Outcome {
        let Some(code) = args["code"].as_str() else {
            return Outcome::err("missing argument `code` (a string)");
        };
        let stop = tuse_lua::Stop::new(
            cancel.clone(),
            Some(Instant::now() + Duration::from_secs(secs)),
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

/// `timeout` of the call (seconds, no upper limit), else `settings.tool_timeout`.
fn timeout_of(args: &serde_json::Value, config: &tuse_config::Config) -> u64 {
    let own = args["timeout"]
        .as_f64()
        .filter(|t| *t > 0.0)
        .map(|t| t.ceil() as u64);
    own.unwrap_or(config.settings.tool_timeout).max(1)
}
