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
use tuse_py::{Ctx, MontyObject, NEVER};

type Vars = Vec<(String, MontyObject)>;
pub struct HookQueue {
    pub tx: Sender<Vars>,
    pub done: Receiver<()>,
    pub stale: Arc<AtomicBool>,
}
impl HookQueue {
    pub fn new(session: Weak<Session>, config: Arc<Config>, name: String, run: String) -> Self {
        let (tx, rx) = mpsc::channel();
        let (finish, done) = mpsc::channel();
        let stale = Arc::new(AtomicBool::new(false));
        let cancelled = stale.clone();
        std::thread::spawn(move || {
            let make = || python_for(session.clone(), &config, true);
            let mut py = make();
            for vars in rx {
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                let ctx = Ctx::new(
                    &NEVER,
                    Instant::now() + Duration::from_secs(config.settings.hook_timeout.max(1)),
                );
                let out = py.run(&run, &ctx, vars);
                if !out.ok
                    && let Some(s) = session.upgrade()
                {
                    s.emit(Event::Notice(format!(
                        "hook {name}: {}",
                        last_line(&out.text)
                    )));
                }
                if ctx.expired() {
                    py = make();
                }
            }
            let _ = finish.send(());
        });
        Self { tx, done, stale }
    }
}

impl Session {
    pub(crate) fn finish_hooks(self: &Arc<Self>) {
        self.hook("session_end", crate::hooks::Data::default());
        let config = self.config();
        let deadline =
            Instant::now() + std::time::Duration::from_secs(config.settings.hook_timeout.max(1));
        let queues = self.hooks.lock().unwrap().take().unwrap_or_default();
        let mut waiting = vec![];
        for (name, q) in queues {
            drop(q.tx);
            if config
                .hooks
                .get(&name)
                .is_some_and(|h| h.events.iter().any(|e| e == "session_end"))
            {
                waiting.push(q.done);
            } else {
                q.stale.store(true, std::sync::atomic::Ordering::Release);
            }
        }
        for done in waiting {
            let _ = done.recv_timeout(deadline.saturating_duration_since(Instant::now()));
        }
    }
}
