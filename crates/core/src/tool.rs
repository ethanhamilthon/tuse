use crate::event::Event;
use crate::hooks::Data;
use crate::id;
use crate::record::Record;
use crate::session::Session;
use crate::tools::{self, Output};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tuse_api::ToolCall;
use tuse_py::Ctx;

impl Session {
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
        let input = tools::input_of(&call.name, &args, &call.arguments);
        self.emit(Event::ToolStart {
            id: call.id.clone(),
            tool: call.name.clone(),
            input: input.clone(),
        });
        self.hook(
            "tool_start",
            Data {
                tool: Some(call.name.clone()),
                input: Some(input.clone()),
                ..Default::default()
            },
        );
        let (out, code) = if parsed.is_err() {
            (
                Output::err(format!(
                    "the arguments are not valid JSON: {}",
                    call.arguments
                )),
                Some(1),
            )
        } else if call.name == "python" {
            self.python_call(&args, cancel, config)
        } else if tools::NAMES.contains(&call.name.as_str()) {
            let out = tools::run(&call.name, &args, &self.cwd, config);
            let code = Some(i32::from(!out.ok));
            (out, code)
        } else {
            (
                Output::err(format!(
                    "unknown tool `{}`; the tools are {}",
                    call.name,
                    tools::NAMES.join(", ")
                )),
                Some(1),
            )
        };
        let text = tuse_py::truncate(&out.text, config.settings.max_output);
        let Some(mut st) = self.lock_live(generation) else {
            return false;
        };
        st.calls += 1;
        let r = Record::Tool {
            id: call.id.clone(),
            tool: call.name.clone(),
            input: input.clone(),
            output: text.clone(),
            code,
            images: out.images,
            at: id::now(),
        };
        self.push(&mut st, r);
        drop(st);
        self.emit(Event::ToolEnd {
            id: call.id.clone(),
            output: text.clone(),
            code,
        });
        self.hook(
            "tool_end",
            Data {
                tool: Some(call.name.clone()),
                input: Some(input),
                output: Some(text),
                code,
                ..Default::default()
            },
        );
        true
    }

    /// Runs `code` in the session's interpreter. Exit code 0 is success, 1 an
    /// error or a timeout, `None` a cancel.
    fn python_call(
        self: &Arc<Self>,
        args: &serde_json::Value,
        cancel: &AtomicBool,
        config: &tuse_config::Config,
    ) -> (Output, Option<i32>) {
        let Some(code) = args["code"].as_str() else {
            return (Output::err("missing argument `code` (a string)"), Some(1));
        };
        let secs = args["timeout"]
            .as_u64()
            .unwrap_or(config.settings.tool_timeout)
            .max(1);
        let ctx = Ctx {
            cancel,
            deadline: Instant::now() + Duration::from_secs(secs),
        };
        let mut slot = self.python.lock().unwrap();
        let py = slot.get_or_insert_with(|| self.new_python(config));
        let o = py.run(code, &ctx, vec![]);
        drop(slot);
        let cancelled = cancel.load(std::sync::atomic::Ordering::Relaxed);
        let exit = if o.ok {
            Some(0)
        } else if cancelled {
            None
        } else {
            Some(1)
        };
        let out = if o.ok {
            Output::ok(o.text)
        } else {
            Output::err(o.text)
        };
        (out, exit)
    }
}
