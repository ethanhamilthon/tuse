use crate::event::Event;
use crate::id;
use crate::record::{FileDiff, Record};
use crate::session::Session;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tuse_api::ToolCall;
use tuse_py::{Ctx, Python};

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
    fn err(text: impl AsRef<str>, max: usize) -> Self {
        Self {
            text: tuse_py::truncate(text.as_ref(), max),
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
        cancel: &AtomicBool,
        call: &ToolCall,
        config: &tuse_config::Config,
    ) -> bool {
        let parsed = serde_json::from_str::<serde_json::Value>(&call.arguments);
        let mut args = parsed.as_ref().cloned().unwrap_or_default();
        let alias = crate::alias::code(&call.name, &args);
        if let Some(code) = &alias {
            args["code"] = code.clone().into();
        }
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
        let max = config.settings.max_output;
        let out = if parsed.is_err() {
            Outcome::err(
                format!("the arguments are not valid JSON: {}", call.arguments),
                max,
            )
        } else if call.name == TOOL || alias.is_some() {
            self.python_call(&args, cancel, config)
        } else {
            Outcome::err(
                format!(
                    "unknown tool `{}`; the only tool is `{TOOL}`: call functions such as `read` or `ask` inside its `code`",
                    call.name
                ),
                max,
            )
        };
        // `python_call` output is already cut to `max_output` by the runtime.
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

    /// Runs `code` in the session's interpreter.
    fn python_call(
        self: &Arc<Self>,
        args: &serde_json::Value,
        cancel: &AtomicBool,
        config: &tuse_config::Config,
    ) -> Outcome {
        let Some(code) = args["code"].as_str() else {
            return Outcome::err(
                "missing argument `code` (a string)",
                config.settings.max_output,
            );
        };
        let secs = args["timeout"]
            .as_u64()
            .unwrap_or(config.settings.tool_timeout)
            .max(1);
        let ctx = Ctx::new(cancel, Instant::now() + Duration::from_secs(secs)).adopting();
        let mut slot = self.python.lock().unwrap();
        let o = run_in_slot(
            &mut slot,
            || self.new_python(config),
            code,
            &ctx,
            config.settings.max_output,
        );
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

const RESTART_NOTE: &str = "\n[the interpreter crashed; it restarts from scratch on the next call, so all variables and imports are lost]";

/// Runs `code` in the slot's interpreter (made on demand). A crashed interpreter is
/// dropped so the next call gets a clean one.
fn run_in_slot(
    slot: &mut Option<Python>,
    make: impl FnOnce() -> Python,
    code: &str,
    ctx: &Ctx,
    max: usize,
) -> tuse_py::Output {
    let py = slot.get_or_insert_with(make);
    let mut o = py.run(code, ctx, vec![]);
    if !py.is_alive() {
        *slot = None;
        o.text.push_str(RESTART_NOTE);
        o.text = tuse_py::truncate(&o.text, max);
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuse_py::{Args, Fail, Host, MontyObject, NEVER};

    struct Boom;
    impl Host for Boom {
        fn functions(&self) -> &'static [&'static str] {
            &["boom"]
        }
        fn call(&self, _: &str, _: &Args, _: &Ctx) -> Result<MontyObject, Fail> {
            panic!("boom")
        }
        fn cwd(&self) -> std::path::PathBuf {
            std::env::temp_dir()
        }
        fn env(&self) -> Vec<(String, String)> {
            vec![]
        }
        fn module_dirs(&self) -> Vec<std::path::PathBuf> {
            vec![]
        }
    }

    #[test]
    fn a_crashed_interpreter_is_replaced_on_the_next_call() {
        let ctx = Ctx::new(&NEVER, Instant::now() + Duration::from_secs(10));
        let make = || Python::new(Arc::new(Boom), 1000);
        let mut slot = None;
        assert!(run_in_slot(&mut slot, make, "x = 1", &ctx, 4096).ok);
        let o = run_in_slot(&mut slot, make, "boom()", &ctx, 4096);
        assert!(!o.ok && o.text.contains("lost"), "{}", o.text);
        assert!(slot.is_none());
        let o = run_in_slot(&mut slot, make, "x", &ctx, 4096);
        assert!(!o.ok && o.text.contains("NameError"), "{}", o.text);
        assert!(slot.is_some());
    }
}
