use crate::app::App;
use crate::render::duration;
use crate::style::{LABEL_SEP, SPINNER, STATUS_MIN_MS, STEERING_LABEL};
use std::time::{Duration, Instant};

/// The action shown in the working indicator. A new action replaces the shown one, but
/// every shown action stays for at least `STATUS_MIN_MS`; the newest one wins.
pub struct Hold {
    shown: String,
    since: Instant,
    latest: String,
}

impl Default for Hold {
    fn default() -> Self {
        Self {
            shown: String::new(),
            since: Instant::now(),
            latest: String::new(),
        }
    }
}

impl Hold {
    pub fn set(&mut self, text: String) {
        self.latest = text;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Moves to the newest action when the shown one is old enough; true when it changed.
    pub fn tick(&mut self) -> bool {
        let old = self.since.elapsed() >= Duration::from_millis(STATUS_MIN_MS);
        if self.shown == self.latest || !(old || self.shown.is_empty()) {
            return false;
        }
        self.shown = self.latest.clone();
        self.since = Instant::now();
        true
    }
}

/// The working indicator: spinner, time and the current action; empty when idle.
pub fn working(app: &App) -> String {
    let Some((elapsed, _)) = app.session.turn_info() else {
        return String::new();
    };
    if app.steering.is_some() {
        return STEERING_LABEL.to_string();
    }
    let spin = SPINNER[app.tick % SPINNER.len()];
    if app.status.shown.is_empty() {
        return format!("{spin} {}", duration(elapsed));
    }
    format!(
        "{spin} {}{LABEL_SEP}{}",
        duration(elapsed),
        app.status.shown
    )
}

/// The model indicator: model, effort, context use and cost.
pub fn model(app: &App) -> String {
    let config = app.session.config();
    let (model, effort) = app.session.model();
    let mut parts = vec![config.display_name(&model), effort];
    let stats = app.session.stats();
    let window = config.model_def(&model).map_or(0, |d| d.window);
    if window > 0 || stats.context > 0 {
        parts.push(crate::maxsize::usage(stats.context, window));
    }
    if stats.priced {
        parts.push(format!("${:.2}", stats.cost));
    }
    parts.join(LABEL_SEP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_action_stays_for_the_minimum_time() {
        let mut h = Hold::default();
        h.set("reading a...".into());
        assert!(h.tick());
        assert_eq!(h.shown, "reading a...");
        h.set("writing b...".into());
        assert!(!h.tick(), "too early to replace");
        h.since -= Duration::from_millis(STATUS_MIN_MS);
        assert!(h.tick());
        assert_eq!(h.shown, "writing b...");
    }
}
