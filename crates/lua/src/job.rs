use mlua::{Lua, MultiValue, Thread, Value};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Stop {
    pub cancel: Arc<AtomicBool>,
    deadline: Option<Instant>,
    handoff: Option<(String, Arc<AtomicBool>)>,
}

impl Stop {
    pub fn new(cancel: Arc<AtomicBool>, deadline: Option<Instant>) -> Self {
        Self {
            cancel,
            deadline,
            handoff: None,
        }
    }

    pub fn transferable(mut self, name: String, requested: Arc<AtomicBool>) -> Self {
        self.handoff = Some((name, requested));
        self
    }

    fn handoff_name(&self) -> Option<&str> {
        let (name, requested) = self.handoff.as_ref()?;
        (self.reason().is_none() && requested.load(Ordering::Acquire)).then_some(name)
    }

    fn resume_after(&mut self, rem: Duration) {
        self.deadline = Some(Instant::now() + rem);
    }

    fn pause(&mut self) -> Option<Duration> {
        self.deadline
            .take()
            .map(|d| d.saturating_duration_since(Instant::now()))
    }

    pub fn reason(&self) -> Option<&'static str> {
        if self.cancel.load(Ordering::Relaxed) {
            Some("cancelled")
        } else if self.deadline.is_some_and(|d| Instant::now() >= d) {
            Some("timed out")
        } else {
            None
        }
    }
}

pub enum Out {
    Buf(String),
    File(std::fs::File),
}

impl Out {
    pub fn put(&mut self, s: &str) {
        match self {
            Out::Buf(b) => b.push_str(s),
            Out::File(f) => {
                let _ = f.write_all(s.as_bytes());
            }
        }
    }
}

pub struct Done {
    pub ok: bool,
    pub text: String,
    pub stop: Option<&'static str>,
    pub handed_off: bool,
    pub diffs: Vec<(String, String)>,
    pub images: Vec<String>,
}

pub struct ExtWait {
    pub rx: std::sync::mpsc::Receiver<Result<serde_json::Value, String>>,
    pub pause_timeout: bool,
    pub on_cancel: Option<Box<dyn FnOnce() + Send>>,
}

impl Done {
    pub fn failed(text: String) -> Self {
        Self {
            ok: false,
            text,
            stop: None,
            handed_off: false,
            diffs: vec![],
            images: vec![],
        }
    }
}

pub type OnDone = Box<dyn FnOnce(&Lua, Done)>;

pub struct Job {
    stop: Stop,
    out: Out,
    diffs: Vec<(String, String)>,
    images: Vec<String>,
    is_hook: bool,
    is_lua_hook: bool,
    thread: Thread,
    args: MultiValue,
    done: Option<OnDone>,
    task: bool,
    resumed: bool,
    code: Option<Arc<str>>,
    returning: bool,
    pub(crate) nested_yield: Option<Thread>,
}

impl Job {
    pub fn new(
        lua: &Lua,
        f: mlua::Function,
        args: MultiValue,
        stop: Stop,
        out: Out,
        done: impl FnOnce(&Lua, Done) + 'static,
    ) -> mlua::Result<Self> {
        Ok(Self {
            stop,
            out,
            diffs: vec![],
            images: vec![],
            is_hook: false,
            is_lua_hook: false,
            thread: lua.create_thread(f)?,
            args,
            done: Some(Box::new(done)),
            task: false,
            resumed: false,
            code: None,
            returning: false,
            nested_yield: None,
        })
    }

    pub(crate) fn on_done(&mut self, done: impl FnOnce(&Lua, Done) + 'static) {
        self.done = Some(Box::new(done));
    }

    pub fn after_done(&mut self, f: impl FnOnce() + 'static) {
        let done = self.done.take();
        self.on_done(move |lua, result| {
            if let Some(done) = done {
                done(lua, result);
            }
            f();
        });
    }

    pub fn hooked(mut self, lua_hook: bool) -> Self {
        self.is_hook = true;
        self.is_lua_hook = lua_hook;
        self
    }

    pub fn returning(mut self) -> Self {
        self.returning = true;
        self
    }

    pub fn with_code(mut self, code: Option<Arc<str>>) -> Self {
        self.code = code;
        self
    }

    pub fn code(&self) -> Option<Arc<str>> {
        self.code.clone()
    }

    pub fn print(&mut self, s: &str) {
        self.out.put(s);
    }

    pub fn add_diff(&mut self, path: String, diff: String) {
        if !self.task {
            self.diffs.push((path, diff));
        }
    }

    pub fn add_image(&mut self, url: String) {
        if !self.task {
            self.images.push(url);
        }
    }

    pub fn is_hook(&self) -> bool {
        self.is_hook
    }

    pub fn is_lua_hook(&self) -> bool {
        self.is_lua_hook
    }

    pub fn background(&self) -> bool {
        self.task
    }

    pub fn as_task(mut self) -> Self {
        self.task = true;
        self
    }

    pub(crate) fn thread(&self) -> Thread {
        self.thread.clone()
    }

    pub(crate) fn take_args(&mut self) -> MultiValue {
        self.resumed = true;
        std::mem::replace(&mut self.args, MultiValue::new())
    }

    pub(crate) fn wake(&mut self, args: MultiValue, paused: Option<Duration>) {
        if let Some(rem) = paused {
            self.stop.resume_after(rem);
        }
        self.args = args;
    }

    pub(crate) fn stop_reason(&self) -> Option<&'static str> {
        self.stop.reason()
    }

    pub(crate) fn wants_handoff(&self) -> bool {
        self.resumed && self.stop.handoff_name().is_some()
    }

    pub(crate) fn handoff(&mut self, lua: &Lua) -> bool {
        if !self.resumed {
            return false;
        }
        let Some(name) = self.stop.handoff_name().map(str::to_owned) else {
            return false;
        };
        self.stop.handoff = None;
        let host = crate::host(lua);
        let code = self.code.as_deref().unwrap_or_default().to_string();
        let Ok(t) = host.task_start(&name, &code, true) else {
            return false;
        };
        let mut out = Out::File(t.log);
        if let Out::Buf(buf) = &self.out {
            out.put(buf);
        }
        self.out = out;
        self.stop = Stop::new(t.cancel, None);
        self.task = true;
        let done = Done {
            ok: false,
            text: format!("[continued in background: {}]", t.id),
            stop: Some("cancelled"),
            handed_off: true,
            diffs: std::mem::take(&mut self.diffs),
            images: std::mem::take(&mut self.images),
        };
        if let Some(on_done) = self.done.take() {
            on_done(lua, done);
        }
        host.task_track(&t.id, &name);
        self.done = Some(Box::new(move |_, done| host.task_end(&t.id, done)));
        true
    }

    pub(crate) fn pause_timeout(&mut self) -> Option<Duration> {
        self.stop.pause()
    }

    pub(crate) fn finish(mut self, lua: &Lua, r: Result<MultiValue, String>) {
        let r = match r {
            Ok(vals) if self.returning => match vals.into_iter().next() {
                Some(Value::String(s)) => {
                    self.out = Out::Buf(s.to_string_lossy());
                    Ok(MultiValue::new())
                }
                other => Err(format!(
                    "must return a string, got {}",
                    other.map_or("nil", |v| v.type_name())
                )),
            },
            r => r,
        };
        if self.returning && r.is_err() {
            self.out = Out::Buf(String::new());
        }
        let stop = self.stop.reason();
        let ok = r.is_ok();
        let tail = match r {
            Ok(vals) => render(lua, vals),
            Err(e) => match stop {
                Some(why) if e.contains(why) => format!("[{why}]"),
                _ => e,
            },
        };
        let mut text = match std::mem::replace(&mut self.out, Out::Buf(String::new())) {
            Out::Buf(b) => b,
            Out::File(mut f) => {
                if !tail.is_empty() {
                    let _ = writeln!(f, "{tail}");
                }
                String::new()
            }
        };
        if !tail.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&tail);
        }
        let done = Done {
            ok: ok && stop.is_none(),
            text,
            stop: if ok { None } else { stop },
            handed_off: false,
            diffs: std::mem::take(&mut self.diffs),
            images: std::mem::take(&mut self.images),
        };
        if let Some(f) = self.done.take() {
            f(lua, done);
        }
    }
}

fn render(lua: &Lua, vals: MultiValue) -> String {
    if vals.iter().all(Value::is_nil) {
        return String::new();
    }
    let f: Option<mlua::Function> = lua.named_registry_value("tuse.render").ok();
    f.and_then(|f| f.call::<String>(vals).ok())
        .unwrap_or_default()
}
