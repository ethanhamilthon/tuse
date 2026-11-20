use crate::event::Event;
use crate::hooks::Data;
use crate::id;
use crate::record::{FileDiff, Record};
use crate::session::Session;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tuse_api::ToolCall;
use tuse_py::Ctx;

/// The only tool.
pub const TOOL: &str = "python";

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
        self.state.lock().unwrap().status = text.clone();
        self.emit(Event::Status(text));
    }

    /// Runs one tool call and records its result. False when the turn went stale.
    pub(crate) fn tool(
        self: &Arc<Self>,
        generation: u64,
        cancel: &AtomicBool,
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
        self.emit(Event::ToolStart {
            id: call.id.clone(),
            tool: call.name.clone(),
            input: input.clone(),
            title: title.clone(),
        });
        self.hook(
            "tool_start",
            Data {
                tool: Some(call.name.clone()),
                input: Some(input.clone()),
                title: Some(title.clone()),
                ..Default::default()
            },
        );
        let out = if parsed.is_err() {
            Outcome::err(format!(
                "the arguments are not valid JSON: {}",
                call.arguments
            ))
        } else if call.name == TOOL {
            self.python_call(&args, cancel, config)
        } else {
            Outcome::err(format!(
                "unknown tool `{}`; the only tool is `{TOOL}`",
                call.name
            ))
        };
        let text = tuse_py::truncate(&out.text, config.settings.max_output);
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
        st.status.clear();
        drop(st);
        self.emit(Event::Status(String::new()));
        self.emit(Event::ToolEnd {
            id: call.id.clone(),
            output: text.clone(),
            code: out.code,
            diffs: out.diffs,
        });
        self.hook(
            "tool_end",
            Data {
                tool: Some(call.name.clone()),
                input: Some(input),
                title: Some(title),
                output: Some(text),
                code: out.code,
                ..Default::default()
            },
        );
        true
    }

    /// Runs `code` in the session's interpreter.
    fn python_call(
        self: &Arc<Self>,
        args: &serde_json::Value,
        cancel: &AtomicBool,
        config: &tuse_config::Config,
    ) -> Outcome {
        let Some(code) = args["code"].as_str() else {
            return Outcome::err("missing argument `code` (a string)");
        };
        let secs = args["timeout"]
            .as_u64()
            .unwrap_or(config.settings.tool_timeout)
            .max(1);
        let ctx = Ctx::new(cancel, Instant::now() + Duration::from_secs(secs)).adopting();
        let mut slot = self.python.lock().unwrap();
        let py = slot.get_or_insert_with(|| self.new_python(config));
        let o = py.run(code, &ctx, vec![]);
        drop(slot);
        let cancelled = cancel.load(std::sync::atomic::Ordering::Relaxed);
        Outcome {
            code: if o.ok {
                Some(0)
            } else if cancelled {
                None
            } else {
                Some(1)
            },
            text: o.text,
            images: ctx.take_images(),
            diffs: ctx
                .take_diffs()
                .into_iter()
                .map(|(path, diff)| FileDiff { path, diff })
                .collect(),
        }
    }
}
