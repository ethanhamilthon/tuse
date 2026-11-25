use crate::convo::record::Record;
use crate::session::Session;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tuse_lua::mlua::{Function, Lua, MultiValue};
use tuse_lua::{Done, Job, Out, Stop};

pub use tuse_config::HOOK_EVENTS as EVENTS;

#[derive(Default)]
pub(crate) struct Lane {
    busy: bool,
    queue: Vec<(Value, std::sync::mpsc::Sender<()>)>,
    cancel: Arc<AtomicBool>,
}

pub(crate) type Runs = Arc<Mutex<HashMap<String, Lane>>>;

fn listens(hook: &tuse_config::Hook, event: &str) -> bool {
    hook.events.iter().any(|e| e == event)
}

impl Session {
    pub(crate) fn hook(
        self: &Arc<Self>,
        event: &str,
        payload: Value,
    ) -> std::sync::mpsc::Receiver<()> {
        let (done, completion) = std::sync::mpsc::channel();
        if !self.hooks_on.load(Ordering::Acquire) {
            return completion;
        }
        let names: Vec<String> = self
            .config()
            .hooks
            .iter()
            .filter(|(_, h)| listens(h, event))
            .map(|(n, _)| n.clone())
            .collect();
        let payload = crate::text::compact(payload);
        for name in names {
            let mut runs = self.hook_runs.lock().unwrap();
            let lane = runs.entry(name.clone()).or_default();
            lane.queue.push((payload.clone(), done.clone()));
            if !lane.busy {
                lane.busy = true;
                drop(runs);
                self.next_hook(name);
            }
        }
        completion
    }

    fn next_hook(self: &Arc<Self>, name: String) {
        let ((vars, completion), cancel) = {
            let mut runs = self.hook_runs.lock().unwrap();
            let Some(lane) = runs.get_mut(&name) else {
                return;
            };
            if lane.queue.is_empty() {
                lane.busy = false;
                return;
            }
            lane.cancel = Arc::default();
            (lane.queue.remove(0), lane.cancel.clone())
        };
        let is_lua_hook = matches!(vars["event"].as_str(), Some("lua_start" | "lua_end"));
        let stop = Stop::new(cancel, None);
        let weak = Arc::downgrade(self);
        let started = Instant::now();
        self.vm.post(move |lua| {
            let s2 = weak.clone();
            let finished = completion.clone();
            let (n2, v2) = (name.clone(), vars.clone());
            let done = move |_: &Lua, d: Done| {
                let _finished = finished;
                if let Some(s) = s2.upgrade() {
                    s.hook_done(&n2, &v2, d, started);
                }
            };
            let start = || -> tuse_lua::mlua::Result<()> {
                let f: Function = crate::scripting::registry::code_of(lua, "hooks", &name)
                    .ok_or_else(|| tuse_lua::mlua::Error::runtime("run is gone"))?;
                let arg = tuse_lua::to_lua(lua, &vars)?;
                let job = Job::new(
                    lua,
                    f,
                    MultiValue::from_vec(vec![arg]),
                    stop,
                    Out::Buf(String::new()),
                    done,
                )?
                .hooked(is_lua_hook);
                tuse_lua::start(lua, job);
                Ok(())
            };
            if let Err(e) = start()
                && let Some(s) = weak.upgrade()
            {
                let d = Done {
                    ok: false,
                    text: tuse_lua::message(&e),
                    stop: None,
                    handed_off: false,
                    diffs: vec![],
                    images: vec![],
                };
                s.hook_done(&name, &vars, d, started);
            }
        });
    }

    fn hook_done(self: &Arc<Self>, name: &str, vars: &Value, d: Done, started: Instant) {
        let error = (!d.ok).then(|| crate::text::error_line(&d.text));
        let notice = error.as_ref().map(|e| format!("hook {name}: {e}"));
        let r = Record::Hook {
            name: name.to_string(),
            event: vars["event"].as_str().unwrap_or("").to_string(),
            vars: vars.clone(),
            error,
            ms: started.elapsed().as_millis() as u64,
        };
        let mut st = self.state.lock().unwrap();
        let mut records = vec![r];
        if let Some(text) = &notice {
            records.push(Record::Notice { text: text.clone() });
        }
        self.record(&mut st, &records, notice.map(crate::Event::Notice));
        drop(st);
        self.next_hook(name.to_string());
        if self.reload_after_hook.swap(false, Ordering::AcqRel) {
            let s = self.clone();
            std::thread::spawn(move || {
                let _ = s.reload();
            });
        }
    }

    pub(crate) fn wait_lifecycle(&self) {
        if let Some(job) = self.lifecycle_job.lock().unwrap().take() {
            let _ = job.join();
        }
    }

    pub(crate) fn finish_hooks(self: &Arc<Self>, event: Option<&'static str>) {
        self.ui_cancel_hooks(true);
        self.wait_lifecycle();
        if let Some(event) = event {
            self.simple_hook(event);
        }
        self.hooks_on.store(false, Ordering::Release);
        while self.hook_runs.lock().unwrap().values().any(|l| l.busy) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub(crate) fn stop_hook_runs(&self) {
        for lane in self.hook_runs.lock().unwrap().values() {
            lane.cancel.store(true, Ordering::Relaxed);
        }
    }
}
