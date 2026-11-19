use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// What ends a run early: the user's cancel or the call's deadline.
pub struct Ctx<'a> {
    pub cancel: &'a AtomicBool,
    deadline: Mutex<Instant>,
    /// Whether a timed out `sh` may be handed to the host as a background task.
    pub adopt: bool,
    /// Files changed by host functions during this run: (path, diff).
    diffs: Mutex<Vec<(String, String)>>,
    /// `data:` URLs the model should see next to the result.
    images: Mutex<Vec<String>>,
}

/// Time a run keeps after a timed out `sh` became a task, so the code can still print.
const ADOPT_GRACE: Duration = Duration::from_secs(3);

impl<'a> Ctx<'a> {
    pub fn new(cancel: &'a AtomicBool, deadline: Instant) -> Self {
        Self {
            cancel,
            deadline: Mutex::new(deadline),
            adopt: false,
            diffs: Mutex::default(),
            images: Mutex::default(),
        }
    }

    pub fn adopting(mut self) -> Self {
        self.adopt = true;
        self
    }

    pub fn deadline(&self) -> Instant {
        *self.deadline.lock().unwrap()
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline()
    }

    pub fn stopped(&self) -> bool {
        self.cancelled() || self.expired()
    }

    pub fn remaining(&self) -> Duration {
        self.deadline().saturating_duration_since(Instant::now())
    }

    /// While the guard lives the deadline does not run: the time spent waiting
    /// for the user (`ask`) is not the agent's time.
    pub fn pause(&self) -> Pause<'_> {
        Pause {
            ctx: self,
            since: Instant::now(),
        }
    }

    pub fn grace(&self) {
        let mut d = self.deadline.lock().unwrap();
        *d = (*d).max(Instant::now() + ADOPT_GRACE);
    }

    pub fn add_diff(&self, path: &str, diff: String) {
        self.diffs.lock().unwrap().push((path.to_string(), diff));
    }

    pub fn take_diffs(&self) -> Vec<(String, String)> {
        std::mem::take(&mut *self.diffs.lock().unwrap())
    }

    pub fn add_image(&self, url: String) {
        self.images.lock().unwrap().push(url);
    }

    pub fn take_images(&self) -> Vec<String> {
        std::mem::take(&mut *self.images.lock().unwrap())
    }
}

pub struct Pause<'a> {
    ctx: &'a Ctx<'a>,
    since: Instant,
}

impl Drop for Pause<'_> {
    fn drop(&mut self) {
        *self.ctx.deadline.lock().unwrap() += self.since.elapsed();
    }
}

/// A cancel flag that is never set, for hooks and tasks.
pub static NEVER: AtomicBool = AtomicBool::new(false);
