use crate::job::{Done, Job, Stop, Wait};
use crate::sh::{Proc, ShOut};
use mlua::{IntoLuaMulti, Lua, MultiValue, Value};
use std::collections::HashMap;
use std::time::Instant;

/// The coroutines of a VM. It lives in the VM's app data, and no borrow of it
/// is held while Lua code runs, so host functions can reach the running job.
#[derive(Default)]
pub struct Sched {
    jobs: Vec<Job>,
    current: Option<Job>,
    procs: HashMap<u64, Proc>,
    next_proc: u64,
}

impl Sched {
    pub fn add(&mut self, job: Job) {
        self.jobs.push(job);
    }

    pub fn current_job(&mut self) -> Option<&mut Job> {
        self.current.as_mut()
    }

    pub fn current_stop(&self) -> Option<&Stop> {
        self.current.as_ref().map(|j| &j.stop)
    }

    pub fn add_proc(&mut self, p: Proc) -> u64 {
        self.next_proc += 1;
        self.procs.insert(self.next_proc, p);
        self.next_proc
    }

    pub fn take_proc(&mut self, key: u64) -> Option<Proc> {
        self.procs.remove(&key)
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

/// `(true, value)` resumes a waiting coroutine with a result, `(false, msg)` with an error.
fn reply(lua: &Lua, r: Result<Value, String>) -> MultiValue {
    match r {
        Ok(v) => (true, v).into_lua_multi(lua),
        Err(e) => (false, e).into_lua_multi(lua),
    }
    .unwrap_or_default()
}

pub fn sh_value(lua: &Lua, o: ShOut) -> Value {
    let t = || -> mlua::Result<Value> {
        let t = lua.create_table()?;
        t.set("ok", o.code == Some(0))?;
        t.set("code", o.code)?;
        t.set("timed_out", o.timed_out)?;
        t.set("out", o.out)?;
        let mt: mlua::Table = lua.named_registry_value("tuse.sh")?;
        t.set_metatable(Some(mt))?;
        Ok(Value::Table(t))
    };
    t().unwrap_or(Value::Nil)
}

/// Resumes every job whose wait is over. Returns false when nothing was ready.
pub fn tick(lua: &Lua) -> bool {
    let ready = {
        let Some(mut s) = lua.app_data_mut::<Sched>() else {
            return false;
        };
        let jobs = std::mem::take(&mut s.jobs);
        let mut ready = vec![];
        for mut job in jobs {
            let stop = job.stop.reason();
            let wake = match (&job.wait, stop) {
                (Wait::Ready(_), _) => None,
                (Wait::Sh(k), Some(why)) => {
                    if let Some(mut p) = s.procs.remove(k) {
                        p.kill();
                    }
                    Some(Err(why.to_string()))
                }
                (_, Some(why)) => Some(Err(why.to_string())),
                (Wait::Sleep(t), None) => (Instant::now() >= *t).then_some(Ok(Value::Nil)),
                (Wait::Sh(k), None) => {
                    let k = *k;
                    let out = s.procs.get_mut(&k).and_then(Proc::poll);
                    out.map(|o| {
                        s.procs.remove(&k);
                        Ok(sh_value(lua, o))
                    })
                }
            };
            if let Some(r) = wake {
                job.wait = Wait::Ready(reply(lua, r));
            }
            if matches!(job.wait, Wait::Ready(_)) {
                ready.push(job);
            } else {
                s.jobs.push(job);
            }
        }
        ready
    };
    let any = !ready.is_empty();
    for job in ready {
        resume(lua, job);
    }
    any
}

fn resume(lua: &Lua, mut job: Job) {
    let Wait::Ready(args) = std::mem::replace(&mut job.wait, Wait::Ready(MultiValue::new())) else {
        return;
    };
    let thread = job.thread.clone();
    lua.app_data_mut::<Sched>().unwrap().current = Some(job);
    let r = thread.resume::<MultiValue>(args);
    let mut job = lua.app_data_mut::<Sched>().unwrap().current.take().unwrap();
    match r {
        Ok(vals) if thread.is_resumable() => {
            job.wait = parse_wait(lua, &vals);
            lua.app_data_mut::<Sched>().unwrap().jobs.push(job);
        }
        Ok(vals) => finish(lua, job, Ok(vals)),
        Err(e) => finish(lua, job, Err(crate::message(&e))),
    }
}

/// What a coroutine waits for: `coroutine.yield(SCHED, kind, arg)` from `sleep` or `sh`.
/// A bare `coroutine.yield()` at the top of a job only lets the others run.
fn parse_wait(lua: &Lua, vals: &MultiValue) -> Wait {
    let marker: Option<mlua::Table> = lua.named_registry_value("tuse.sched").ok();
    let is_sched = matches!((vals.front(), &marker), (Some(Value::Table(t)), Some(m)) if t == m);
    if !is_sched {
        return Wait::Ready(MultiValue::new());
    }
    let kind = vals
        .get(1)
        .and_then(|v| v.as_string().map(|s| s.to_string_lossy()))
        .unwrap_or_default();
    let arg = vals.get(2).cloned().unwrap_or(Value::Nil);
    match kind.as_str() {
        "sleep" => {
            let secs = arg
                .as_number()
                .or(arg.as_integer().map(|i| i as f64))
                .unwrap_or(0.0);
            Wait::Sleep(Instant::now() + std::time::Duration::from_secs_f64(secs.max(0.0)))
        }
        "sh" => Wait::Sh(arg.as_integer().unwrap_or(0) as u64),
        _ => Wait::Ready(reply(lua, Err(format!("unknown wait `{kind}`")))),
    }
}

fn finish(lua: &Lua, mut job: Job, r: Result<MultiValue, String>) {
    let stop = job.stop.reason();
    let ok = r.is_ok();
    let tail = match r {
        Ok(vals) => render(lua, vals),
        Err(e) => match stop {
            Some(why) if e.contains(why) => format!("[{why}]"),
            _ => e,
        },
    };
    let mut text = match std::mem::replace(&mut job.out, crate::job::Out::Buf(String::new())) {
        crate::job::Out::Buf(b) => b,
        crate::job::Out::File(mut f) => {
            if !tail.is_empty() {
                use std::io::Write;
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
        diffs: std::mem::take(&mut job.diffs),
        images: std::mem::take(&mut job.images),
    };
    if let Some(f) = job.done.take() {
        f(lua, done);
    }
}

/// The values a job returned, as the agent reads them (`tuse.render` in the prelude).
fn render(lua: &Lua, vals: MultiValue) -> String {
    if vals.iter().all(Value::is_nil) {
        return String::new();
    }
    let f: Option<mlua::Function> = lua.named_registry_value("tuse.render").ok();
    f.and_then(|f| f.call::<String>(vals).ok())
        .unwrap_or_default()
}
