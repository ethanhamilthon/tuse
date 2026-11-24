use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tuse_core::Session;

const GRACE: Duration = Duration::from_secs(2);
const CHECK: Duration = Duration::from_millis(250);

pub struct Watchdog {
    session: Mutex<Arc<Session>>,
    closing: AtomicBool,
}

impl Watchdog {
    pub fn start(session: Arc<Session>, stop: Arc<AtomicBool>) -> Arc<Watchdog> {
        let w = Arc::new(Watchdog {
            session: Mutex::new(session),
            closing: AtomicBool::new(false),
        });
        let w2 = w.clone();
        std::thread::Builder::new()
            .name("tuse-watchdog".into())
            .spawn(move || w2.watch(&stop))
            .expect("spawn watchdog thread");
        w
    }

    pub fn set_session(&self, s: &Arc<Session>) {
        let mut cur = self.session.lock().unwrap();
        if !Arc::ptr_eq(&cur, s) {
            *cur = s.clone();
        }
    }

    pub fn claim_close(&self) -> bool {
        self.closing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn watch(&self, stop: &AtomicBool) {
        let tty = std::io::stdin().is_terminal();
        let mut since: Option<Instant> = None;
        loop {
            std::thread::sleep(CHECK);
            if self.closing.load(Ordering::Acquire) {
                return;
            }
            let gone = stop.load(Ordering::Relaxed) || (tty && !std::io::stdin().is_terminal());
            if !gone {
                since = None;
                continue;
            }
            let t = *since.get_or_insert_with(Instant::now);
            if t.elapsed() < GRACE || !self.claim_close() {
                continue;
            }
            let session = self.session.lock().unwrap().clone();
            session.close();
            crate::restore();
            std::process::exit(0);
        }
    }
}
