//! The tab title (OSC 2: `tuse · <session title>`) and the state indicator
//! (OSC 9;4: working spins, a question pauses, an error is red, otherwise nothing).
use crate::app::App;
use std::time::{Duration, Instant};

/// Saves the terminal's title on start; `RESTORE` brings it back on exit.
pub const SAVE: &str = "\x1b[22;0t";
pub const RESTORE: &str = "\x1b]9;4;0\x1b\\\x1b]2;\x1b\\\x1b[23;0t";

pub fn title(session: &str) -> String {
    let clean: String = session
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        "\x1b]2;tuse\x1b\\".into()
    } else {
        format!("\x1b]2;tuse · {clean}\x1b\\")
    }
}

/// `9;4` states: 0 none, 2 error, 3 indeterminate, 4 paused. Error and paused carry
/// 100%: with 0% some terminals draw a zero-width bar.
pub fn progress(state: u8) -> String {
    match state {
        2 | 4 => format!("\x1b]9;4;{state};100\x1b\\"),
        _ => format!("\x1b]9;4;{state}\x1b\\"),
    }
}

/// Terminals clear the indicator after 15s (Ghostty) or 60s (Kitty) without updates.
const KEEPALIVE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct Osc {
    title: Option<String>,
    progress: Option<String>,
    sent_at: Option<Instant>,
}

impl Osc {
    fn changed(last: &mut Option<String>, s: String) -> Option<String> {
        if last.as_ref() == Some(&s) {
            return None;
        }
        *last = Some(s.clone());
        Some(s)
    }

    pub fn update(&mut self, app: &App) -> Option<Vec<u8>> {
        self.update_at(app, Instant::now())
    }

    fn update_at(&mut self, app: &App, now: Instant) -> Option<Vec<u8>> {
        if !app.session.config().settings.ui.tab_status {
            let sent = self.title.is_some() || self.progress.is_some();
            *self = Osc::default();
            return sent.then(|| RESTORE.as_bytes().to_vec());
        }
        let state = if app.ask_input.is_some() || !app.asks.is_empty() {
            4
        } else if app.session.turn_info().is_some() {
            3
        } else if app.turn_error.is_some() {
            2
        } else {
            0
        };
        let mut out = String::new();
        out.extend(Self::changed(&mut self.title, title(&app.session.title())));
        let due = matches!(state, 3 | 4)
            && self
                .sent_at
                .is_none_or(|t| now.duration_since(t) >= KEEPALIVE);
        if due {
            self.progress = None;
        }
        let sent = Self::changed(&mut self.progress, progress(state));
        if sent.is_some() {
            self.sent_at = Some(now);
        }
        out.extend(sent);
        (!out.is_empty()).then(|| out.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_states_and_dedup() {
        assert_eq!(title(""), "\x1b]2;tuse\x1b\\");
        assert_eq!(
            title(" fix\x1b]0;x\x07 it "),
            "\x1b]2;tuse · fix]0;x it\x1b\\"
        );
        let want = [(0, "0"), (2, "2"), (3, "3"), (4, "4")];
        for (state, code) in want {
            let tail = if state == 2 || state == 4 { ";100" } else { "" };
            assert_eq!(progress(state), format!("\x1b]9;4;{code}{tail}\x1b\\"));
        }
        assert_eq!(RESTORE, "\x1b]9;4;0\x1b\\\x1b]2;\x1b\\\x1b[23;0t");
        let mut app = crate::app::test_app("osc");
        let mut osc = Osc::default();
        let first = String::from_utf8(osc.update(&app).unwrap()).unwrap();
        assert_eq!(first, "\x1b]2;tuse\x1b\\\x1b]9;4;0\x1b\\");
        assert!(osc.update(&app).is_none());
        app.turn_error = Some("boom".into());
        assert_eq!(osc.update(&app).unwrap(), progress(2).into_bytes());
        crate::app::cleanup(app);
    }

    #[test]
    fn keepalive_and_restore() {
        let mut app = crate::app::test_app("osc_keep");
        let mut osc = Osc::default();
        let t0 = Instant::now();
        let s = Duration::from_secs;
        osc.update_at(&app, t0);
        assert!(osc.update_at(&app, t0 + s(30)).is_none()); // idle: no repeats
        app.turn_error = Some("boom".into());
        assert!(osc.update_at(&app, t0 + s(31)).is_some());
        assert!(osc.update_at(&app, t0 + s(60)).is_none()); // error: once
        app.turn_error = None;
        app.ask_input = Some(Default::default());
        let t1 = t0 + s(100);
        assert_eq!(osc.update_at(&app, t1).unwrap(), progress(4).into_bytes());
        assert!(osc.update_at(&app, t1 + s(4)).is_none());
        assert_eq!(
            osc.update_at(&app, t1 + s(5)).unwrap(),
            progress(4).into_bytes()
        );
        assert!(osc.update_at(&app, t1 + s(9)).is_none()); // timer restarted
        assert!(osc.update_at(&app, t1 + s(10)).is_some());
        app.ask_input = None;
        assert_eq!(
            osc.update_at(&app, t1 + s(11)).unwrap(),
            progress(0).into_bytes()
        );
        std::fs::write(
            app.cwd.join("tuse.yaml"),
            "settings:\n  ui:\n    tab_status: false\n",
        )
        .unwrap();
        app.session.reload().unwrap();
        let out = String::from_utf8(osc.update_at(&app, t1 + s(12)).unwrap()).unwrap();
        assert_eq!(out, RESTORE);
        assert!(osc.update_at(&app, t1 + s(13)).is_none());
        crate::app::cleanup(app);
    }
}
