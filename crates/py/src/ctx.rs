use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// What ends a run early: the user's cancel or the call's deadline.
pub struct Ctx<'a> {
    pub cancel: &'a AtomicBool,
    pub deadline: Instant,
}

impl Ctx<'_> {
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn stopped(&self) -> bool {
        self.cancelled() || self.expired()
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}

/// A cancel flag that is never set, for hooks.
pub static NEVER: AtomicBool = AtomicBool::new(false);
