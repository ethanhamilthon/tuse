use crate::{
    Event, Session,
    host::{last_line, python_for},
};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::time::{Duration, Instant};
use tuse_config::Config;
use tuse_py::{Ctx, MontyObject};

type Vars = Vec<(String, MontyObject)>;

/// Variables every hook gets; the trace lists only the event's own.
const COMMON: &[&str] = &["event", "hook", "session", "cwd", "model", "config"];

fn describe(vars: &Vars) -> String {
    vars.iter()
        .filter(|(k, v)| !COMMON.contains(&k.as_str()) && v.type_name() != "NoneType")
        .map(|(k, v)| {
            let v = v.py_repr();
            let short: String = v.chars().take(200).collect();
            let cut = if short.len() < v.len() { "…" } else { "" };
            format!("{k}={short}{cut}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
pub struct HookQueue {
    pub tx: Sender<Vars>,
    pub done: Receiver<()>,
    pub interrupt: Arc<AtomicBool>,
    pub stale: Arc<AtomicBool>,
}
impl HookQueue {
    pub fn new(session: Weak<Session>, config: Arc<Config>, name: String, run: String) -> Self {
        let (tx, rx) = mpsc::channel::<Vars>();
        let (finish, done) = mpsc::channel();
        let stale = Arc::new(AtomicBool::new(false));
        let retired = stale.clone();
        let interrupt = Arc::new(AtomicBool::new(false));
        let cancelled = interrupt.clone();
        std::thread::spawn(move || {
            let make = || python_for(session.clone(), &config, crate::host::Origin::Hook);
            let mut py = make();
            for vars in rx {
                cancelled.store(false, Ordering::Release);
                if retired.load(Ordering::Acquire) {
                    break;
                }
                let ctx = Ctx::new(
                    &cancelled,
                    Instant::now() + Duration::from_secs(config.settings.hook_timeout.max(1)),
                )
                .adopting();
                let event = vars
                    .iter()
                    .find(|(k, _)| k == "event")
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default();
                let described = describe(&vars);
                let started = Instant::now();
                let out = py.run(&run, &ctx, vars);
                if let Some(s) = session.upgrade() {
                    let error = (!out.ok).then(|| last_line(&out.text));
                    if let Some(e) = &error {
                        s.emit(Event::Notice(format!("hook {name}: {e}")));
                    }
                    s.record_hook(crate::record::Record::Hook {
                        name: name.clone(),
                        event,
                        vars: described,
                        error,
                        ms: started.elapsed().as_millis() as u64,
                    });
                }
                if ctx.expired() || !py.is_alive() {
                    py = make();
                }
            }
            let _ = finish.send(());
        });
        Self {
            tx,
            done,
            stale,
            interrupt,
        }
    }
}

impl Session {
    fn record_hook(&self, r: crate::record::Record) {
        let mut st = self.state.lock().unwrap();
        self.push(&mut st, r);
    }

    /// Fires `process_end` and waits up to `hook_timeout` for the hooks that listen to it;
    /// the other queues are dropped. The hooks of the next config start fresh.
    pub(crate) fn finish_hooks(self: &Arc<Self>, reason: &'static str) {
        self.hook(
            "process_end",
            crate::hooks::Data {
                reason: Some(reason),
                ..Default::default()
            },
        );
        let config = self.config();
        let deadline =
            Instant::now() + std::time::Duration::from_secs(config.settings.hook_timeout.max(1));
        let queues = self.hooks.lock().unwrap().take().unwrap_or_default();
        let mut waiting = vec![];
        for (name, q) in queues {
            drop(q.tx);
            if config.hooks.get(&name).is_some_and(|h| {
                h.matches(self.mode.into()) && h.events.iter().any(|e| e == "process_end")
            }) {
                waiting.push((q.done, q.stale, q.interrupt));
            } else {
                q.stale.store(true, Ordering::Release);
                q.interrupt.store(true, Ordering::Release);
            }
        }
        for (done, stale, interrupt) in waiting {
            if done
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .is_err()
            {
                stale.store(true, Ordering::Release);
                interrupt.store(true, Ordering::Release);
            }
        }
    }
}
