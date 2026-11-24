use mlua::{Lua, MultiValue, Thread, Value};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Stop {
    pub cancel: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl Stop {
    pub fn new(cancel: Arc<AtomicBool>, deadline: Option<Instant>) -> Self {
        Self { cancel, deadline }
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
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
    pub diffs: Vec<(String, String)>,
    pub images: Vec<String>,
}

pub struct ExtWait {
    pub rx: std::sync::mpsc::Receiver<Result<serde_json::Value, String>>,
    pub pause_timeout: bool,
    pub on_cancel: Option<Box<dyn FnOnce() + Send>>,
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
        })
    }

    pub fn hooked(mut self, lua_hook: bool) -> Self {
        self.is_hook = true;
        self.is_lua_hook = lua_hook;
        self
    }

    pub fn print(&mut self, s: &str) {
        self.out.put(s);
    }

    pub fn add_diff(&mut self, path: String, diff: String) {
        self.diffs.push((path, diff));
    }

    pub fn add_image(&mut self, url: String) {
        self.images.push(url);
    }

    pub fn is_hook(&self) -> bool {
        self.is_hook
    }

    pub fn is_lua_hook(&self) -> bool {
        self.is_lua_hook
    }

    pub fn stop(&self) -> Stop {
        self.stop.clone()
    }

    pub(crate) fn thread(&self) -> Thread {
        self.thread.clone()
    }

    pub(crate) fn take_args(&mut self) -> MultiValue {
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

    pub(crate) fn pause_timeout(&mut self) -> Option<Duration> {
        self.stop.pause()
    }

    pub(crate) fn finish(mut self, lua: &Lua, r: Result<MultiValue, String>) {
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
