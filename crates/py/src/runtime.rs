use crate::ctx::Ctx;
use crate::host::Host;
use crate::print::Collector;
use crate::{funcs, os, render, sh};
use monty::{MontyRepl, ReplProgress};
use monty_fs::{MountMode, MountTable};
use monty_types::{
    CompileOptions, ExcType, ExtFunctionResult, MontyException, MontyObject, NameLookupResult,
    PrintWriter, ResourceLimits, ResourceTracker,
};
use std::sync::Arc;

/// Output collected before the head and tail cut; more is dropped.
const COLLECT: usize = 4 * 1024 * 1024;
/// Stack of the interpreter thread: deep recursion must not overflow the caller's.
const STACK: usize = 16 * 1024 * 1024;

pub struct Output {
    pub text: String,
    /// False when the code raised, was cancelled or timed out.
    pub ok: bool,
}

/// One interpreter: variables, functions and imports live between `run` calls.
pub struct Python {
    repl: Option<MontyRepl>,
    mounts: MountTable,
    host: Arc<dyn Host>,
    max_output: usize,
}

impl Python {
    pub fn new(host: Arc<dyn Host>, max_output: usize) -> Self {
        let tracker = ResourceTracker::new(ResourceLimits::default());
        let mut mounts = MountTable::new();
        // The whole filesystem, read-write: the agent has `sh` anyway.
        if let Err(e) = mounts.mount("/", "/", MountMode::ReadWrite, None) {
            eprintln!("tuse: cannot mount /: {e}");
        }
        let mut repl = MontyRepl::new("<tuse>", tracker, CompileOptions::default());
        repl.set_cwd(&host.cwd().display().to_string());
        Self {
            repl: Some(repl),
            mounts,
            host,
            max_output,
        }
    }

    pub fn set_max_output(&mut self, max_output: usize) {
        self.max_output = max_output;
    }

    /// Runs `code`; `vars` become globals for this run (hooks pass their event data).
    pub fn run(&mut self, code: &str, ctx: &Ctx, vars: Vec<(String, MontyObject)>) -> Output {
        let Some(repl) = self.repl.take() else {
            return Output {
                text: "the interpreter is gone (an earlier run crashed)".into(),
                ok: false,
            };
        };
        let (host, mounts, max) = (&self.host, &mut self.mounts, self.max_output);
        let joined = std::thread::scope(|s| {
            std::thread::Builder::new()
                .stack_size(STACK)
                .spawn_scoped(s, || drive(repl, host.as_ref(), mounts, ctx, code, vars))
                .map(|h| h.join())
        });
        let (repl, result, printed, dropped) = match joined {
            Ok(Ok(r)) => r,
            _ => {
                return Output {
                    text: "the interpreter crashed".into(),
                    ok: false,
                };
            }
        };
        self.repl = Some(repl);
        let mut text = printed;
        let ok = match result {
            Ok(v) => {
                join(&mut text, &render::value(&v));
                true
            }
            Err(e) => {
                join(&mut text, &error_text(&e, ctx));
                false
            }
        };
        if dropped > 0 {
            text.push_str(&format!("\n[... {dropped} more bytes dropped]"));
        }
        let text = sh::truncate(text.trim_end(), max);
        Output {
            text: if text.is_empty() {
                "[no output]".into()
            } else {
                text
            },
            ok,
        }
    }
}

fn join(text: &mut String, more: &str) {
    if more.is_empty() {
        return;
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(more);
}

fn error_text(e: &MontyException, ctx: &Ctx) -> String {
    match e.exc_type() {
        ExcType::KeyboardInterrupt if ctx.cancelled() => "[cancelled]".into(),
        ExcType::TimeoutError if ctx.expired() => "[timed out]".into(),
        _ => e.to_string(),
    }
}

type Done = (
    MontyRepl,
    Result<MontyObject, MontyException>,
    String,
    usize,
);

/// Feeds one snippet and answers every suspension until it completes.
fn drive(
    mut repl: MontyRepl,
    host: &dyn Host,
    mounts: &mut MountTable,
    ctx: &Ctx,
    code: &str,
    vars: Vec<(String, MontyObject)>,
) -> Done {
    repl.tracker_mut().set_max_feed_duration(ctx.remaining());
    let mut col = Collector::new(COLLECT, ctx);
    let mut inputs = vars;
    for n in funcs::names(host) {
        inputs.push((n.to_string(), MontyObject::function(n, None)));
    }
    let mut progress = repl.feed_start(code, inputs, PrintWriter::Callback(&mut col));
    let result = loop {
        let p = match progress {
            Ok(p) => p,
            Err(e) => break (e.repl, Err(e.error)),
        };
        progress = match p {
            ReplProgress::Complete { repl, value } => break (repl, Ok(value)),
            ReplProgress::FunctionCall(c) => {
                if ctx.stopped() {
                    c.abort(stop_error(ctx), PrintWriter::Callback(&mut col))
                } else {
                    let r: ExtFunctionResult =
                        match funcs::call(host, &c.function_name, &c.args, ctx) {
                            Ok(v) => ExtFunctionResult::Return(v),
                            Err(f) => f.into(),
                        };
                    if ctx.cancelled() {
                        c.abort(stop_error(ctx), PrintWriter::Callback(&mut col))
                    } else {
                        c.resume(r, PrintWriter::Callback(&mut col))
                    }
                }
            }
            ReplProgress::OsCall(c) => {
                if ctx.stopped() {
                    c.abort(stop_error(ctx), PrintWriter::Callback(&mut col))
                } else {
                    c.resume_with(PrintWriter::Callback(&mut col), |call| {
                        os::handle(mounts, host, ctx, call)
                    })
                }
            }
            ReplProgress::NameLookup(l) => {
                l.resume(NameLookupResult::Undefined, PrintWriter::Callback(&mut col))
            }
            ReplProgress::ResolveFutures(f) => {
                let e = MontyException::new(
                    ExcType::RuntimeError,
                    Some("async code is not supported here".into()),
                );
                break (f.into_repl(), Err(e));
            }
        };
    };
    (result.0, result.1, col.buf, col.dropped)
}

fn stop_error(ctx: &Ctx) -> MontyException {
    if ctx.cancelled() {
        MontyException::new(ExcType::KeyboardInterrupt, Some("cancelled".into()))
    } else {
        MontyException::new(ExcType::TimeoutError, Some("the call timed out".into()))
    }
}
