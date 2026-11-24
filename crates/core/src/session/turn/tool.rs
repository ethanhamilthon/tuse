use crate::convo::record::{FileDiff, Record};
use crate::event::Event;
use crate::id;
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
    pub(crate) fn tool(
        self: &Arc<Self>,
        generation: u64,
        cancel: &Arc<AtomicBool>,
        call: &ToolCall,
    ) -> Option<bool> {
        let parsed = serde_json::from_str::<serde_json::Value>(&call.arguments);
        let code = parsed.as_ref().ok().and_then(|args| args["code"].as_str());
        let input = code.unwrap_or(&call.arguments).to_string();
        let mut st = self.lock_live(generation)?;
        self.emit(Event::ToolStart {
            timeout: TIMEOUT_SECS,
            id: call.id.clone(),
            tool: call.name.clone(),
            input: input.clone(),
        });
        if call.name == TOOL
            && let Some(code) = code
        {
            let request = Arc::new(AtomicBool::new(false));
            st.tool_handoff = Some(request.clone());
            drop(st);
            let exec_id = self.next_lua_id();
            let stop = tuse_lua::Stop::new(
                cancel.clone(),
                Some(Instant::now() + Duration::from_secs(TIMEOUT_SECS)),
            )
            .transferable(exec_id.clone(), request);
            let started = Instant::now();
            self.lua_start_hook(exec_id.clone(), "tool", Some(code.to_string()), None);
            let s = self.clone();
            let (owned_call, owned_input) = (call.clone(), input.clone());
            return self
                .vm
                .run_observed(
                    code,
                    "lua",
                    stop,
                    crate::scripting::registry::agent_env,
                    move |d| {
                        s.lua_end_done(&exec_id, "tool", started.elapsed(), &d);
                        let handed_off = d.handed_off;
                        s.tool_done(
                            generation,
                            &owned_call,
                            owned_input,
                            Outcome::from(d),
                            handed_off,
                        )
                    },
                )
                .unwrap_or_else(|| {
                    self.tool_done(
                        generation,
                        call,
                        input,
                        Outcome::err("the lua thread is gone"),
                        false,
                    )
                });
        }
        drop(st);
        let out = if parsed.is_err() {
            Outcome::err(format!(
                "the arguments are not valid JSON: {}",
                call.arguments
            ))
        } else if call.name == TOOL {
            Outcome::err("missing argument `code` (a string)")
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
        self.tool_done(generation, call, input, out, false)
    }

    fn tool_done(
        self: &Arc<Self>,
        generation: u64,
        call: &ToolCall,
        input: String,
        out: Outcome,
        handed_off: bool,
    ) -> Option<bool> {
        let mut st = self.lock_live(generation)?;
        st.tool_handoff = None;
        st.calls += 1;
        self.push(
            &mut st,
            Record::Tool {
                id: call.id.clone(),
                tool: call.name.clone(),
                input,
                output: out.text.clone(),
                code: out.code,
                images: out.images,
                diffs: out.diffs.clone(),
                at: id::now(),
            },
        );
        self.emit(Event::ToolEnd {
            id: call.id.clone(),
            output: out.text,
            code: out.code,
            diffs: out.diffs,
        });
        if st.cancel_pending {
            self.finish_cancel(st);
            return None;
        }
        if handed_off {
            for r in crate::convo::history::missing_results(&st.records) {
                self.push(&mut st, r);
            }
        }
        Some(handed_off)
    }
}

impl From<tuse_lua::Done> for Outcome {
    fn from(d: tuse_lua::Done) -> Self {
        Self {
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
