use crate::job::{ExtWait, Job};
use crate::sh::Proc;
use crate::value::{sh_value, to_lua};
use mlua::{IntoLuaMulti, Lua, MultiValue, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

enum Wait {
    Ready,
    Sleep(Instant),
    Sh(u64),
    Ext { key: u64, paused: Option<Duration> },
    Unknown(String),
}

#[derive(Default)]
pub struct Sched {
    parked: Vec<(Job, Wait)>,
    procs: HashMap<u64, Proc>,
    next_proc: u64,
    waits: HashMap<u64, ExtWait>,
    next_wait: u64,
}

#[derive(Default)]
struct Running(Option<Job>);

impl Running {
    fn enter(&mut self, job: Job) {
        self.0 = Some(job);
    }

    fn leave(&mut self) -> Job {
        self.0.take().unwrap()
    }

    fn with<R>(&mut self, f: impl FnOnce(&mut Job) -> R) -> Option<R> {
        self.0.as_mut().map(f)
    }
}

impl Sched {
    pub fn is_empty(&self) -> bool {
        self.parked.is_empty()
    }

    fn add_proc(&mut self, p: Proc) -> u64 {
        self.next_proc += 1;
        self.procs.insert(self.next_proc, p);
        self.next_proc
    }

    fn take_proc(&mut self, key: u64) -> Option<Proc> {
        self.procs.remove(&key)
    }

    fn add_wait(&mut self, w: ExtWait) -> u64 {
        self.next_wait += 1;
        self.waits.insert(self.next_wait, w);
        self.next_wait
    }

    fn poll(&mut self, lua: &Lua) -> Vec<Job> {
        let parked = std::mem::take(&mut self.parked);
        let mut ready = vec![];
        for (mut job, mut wait) in parked {
            if let Some(reply) = self.step(lua, &wait, job.stop_reason()) {
                let paused = match wait {
                    Wait::Ext { paused, .. } => paused,
                    _ => None,
                };
                job.wake(reply, paused);
                wait = Wait::Ready;
            }
            if matches!(wait, Wait::Ready) {
                ready.push(job);
            } else {
                self.parked.push((job, wait));
            }
        }
        ready
    }

    fn step(&mut self, lua: &Lua, wait: &Wait, stop: Option<&'static str>) -> Option<MultiValue> {
        let wake: Option<Result<Value, String>> = match (wait, stop) {
            (Wait::Ready, _) => None,
            (Wait::Unknown(k), _) => Some(Err(format!("unknown wait `{k}`"))),
            (Wait::Sh(k), Some(why)) => {
                if let Some(mut p) = self.procs.remove(k) {
                    p.kill();
                }
                Some(Err(why.to_string()))
            }
            (Wait::Ext { key, .. }, Some(why)) => {
                if let Some(mut w) = self.waits.remove(key)
                    && let Some(cancel) = w.on_cancel.take()
                {
                    cancel();
                }
                Some(Err(why.to_string()))
            }
            (_, Some(why)) => Some(Err(why.to_string())),
            (Wait::Sleep(t), None) => (Instant::now() >= *t).then_some(Ok(Value::Nil)),
            (Wait::Sh(k), None) => {
                let k = *k;
                let out = self.procs.get_mut(&k).and_then(Proc::poll);
                out.map(|o| {
                    self.procs.remove(&k);
                    Ok(sh_value(lua, o))
                })
            }
            (Wait::Ext { key, .. }, None) => {
                let key = *key;
                match self.waits.get(&key).map(|w| w.rx.try_recv()) {
                    Some(Ok(res)) => {
                        self.waits.remove(&key);
                        Some(res.map(|v| to_lua(lua, &v).unwrap_or(Value::Nil)))
                    }
                    Some(Err(std::sync::mpsc::TryRecvError::Empty)) => None,
                    _ => {
                        self.waits.remove(&key);
                        Some(Err("cancelled".into()))
                    }
                }
            }
        };
        wake.map(|r| reply(lua, r))
    }

    fn pauses_timeout(&self, key: u64) -> bool {
        self.waits.get(&key).is_some_and(|w| w.pause_timeout)
    }

    fn start(lua: &Lua, job: Job) {
        if let Some(mut s) = lua.app_data_mut::<Sched>() {
            s.parked.push((job, Wait::Ready));
        }
    }

    fn resume(lua: &Lua, mut job: Job) {
        let args = job.take_args();
        let thread = job.thread();
        lua.app_data_mut::<Running>().unwrap().enter(job);
        let r = thread.resume::<MultiValue>(args);
        let mut job = lua.app_data_mut::<Running>().unwrap().leave();
        match r {
            Ok(vals) if thread.is_resumable() => {
                let wait = parse_wait(lua, &vals, &mut job);
                lua.app_data_mut::<Sched>()
                    .unwrap()
                    .parked
                    .push((job, wait));
            }
            Ok(vals) => job.finish(lua, Ok(vals)),
            Err(e) => job.finish(lua, Err(crate::message(&e))),
        }
    }
}

pub fn install(lua: &Lua) {
    lua.set_app_data(Sched::default());
    lua.set_app_data(Running::default());
}

pub fn start(lua: &Lua, job: Job) {
    Sched::start(lua, job);
}

pub fn with_run<R>(lua: &Lua, f: impl FnOnce(&mut Job) -> R) -> Option<R> {
    lua.app_data_mut::<Running>()?.with(f)
}

pub fn stopped(lua: &Lua) -> Option<&'static str> {
    with_run(lua, |j| j.stop().reason()).flatten()
}

pub fn is_hook(lua: &Lua) -> bool {
    with_run(lua, |j| j.is_hook()).unwrap_or(false)
}

pub fn is_lua_hook(lua: &Lua) -> bool {
    with_run(lua, |j| j.is_lua_hook()).unwrap_or(false)
}

pub fn add_wait(lua: &Lua, w: ExtWait) -> mlua::Result<u64> {
    let mut s = lua
        .app_data_mut::<Sched>()
        .ok_or_else(|| mlua::Error::runtime("scheduler is gone"))?;
    Ok(s.add_wait(w))
}

pub(crate) fn add_proc(lua: &Lua, p: Proc) -> u64 {
    lua.app_data_mut::<Sched>().unwrap().add_proc(p)
}

pub(crate) fn take_proc(lua: &Lua, key: u64) -> Option<Proc> {
    lua.app_data_mut::<Sched>().unwrap().take_proc(key)
}

pub fn tick(lua: &Lua) -> bool {
    let ready = {
        let Some(mut s) = lua.app_data_mut::<Sched>() else {
            return false;
        };
        s.poll(lua)
    };
    let any = !ready.is_empty();
    for job in ready {
        Sched::resume(lua, job);
    }
    any
}

fn parse_wait(lua: &Lua, vals: &MultiValue, job: &mut Job) -> Wait {
    let marker: Option<mlua::Table> = lua.named_registry_value("tuse.sched").ok();
    let is_sched = matches!((vals.front(), &marker), (Some(Value::Table(t)), Some(m)) if t == m);
    if !is_sched {
        return Wait::Ready;
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
        "wait" => {
            let key = arg.as_integer().unwrap_or(0) as u64;
            let pause = lua
                .app_data_ref::<Sched>()
                .is_some_and(|s| s.pauses_timeout(key));
            let paused = pause.then(|| job.pause_timeout()).flatten();
            Wait::Ext { key, paused }
        }
        _ => Wait::Unknown(kind),
    }
}

fn reply(lua: &Lua, r: Result<Value, String>) -> MultiValue {
    match r {
        Ok(v) => (true, v).into_lua_multi(lua),
        Err(e) => (false, e).into_lua_multi(lua),
    }
    .unwrap_or_default()
}
