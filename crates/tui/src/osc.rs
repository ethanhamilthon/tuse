//! The tab title (OSC 2: `tuse · <session title>`) and the state indicator
//! (OSC 9;4: working spins, a question pauses, an error is red, otherwise nothing).
use crate::app::App;

/// Saves the terminal's title on start; `RESTORE` brings it back on exit.
pub const SAVE: &str = "\x1b[22;0t";
pub const RESTORE: &str = "\x1b]9;4;0;0\x1b\\\x1b]2;\x1b\\\x1b[23;0t";

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

/// `9;4` states: 0 none, 2 error, 3 indeterminate, 4 paused.
pub fn progress(state: u8) -> String {
    format!("\x1b]9;4;{state};0\x1b\\")
}

#[derive(Default)]
pub struct Osc {
    title: Option<String>,
    progress: Option<String>,
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
        if !app.session.config().settings.ui.tab_status {
            *self = Osc::default();
            return None;
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
        out.extend(Self::changed(&mut self.progress, progress(state)));
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
            assert_eq!(progress(state), format!("\x1b]9;4;{code};0\x1b\\"));
        }
        let mut app = crate::app::test_app("osc");
        let mut osc = Osc::default();
        let first = String::from_utf8(osc.update(&app).unwrap()).unwrap();
        assert_eq!(first, "\x1b]2;tuse\x1b\\\x1b]9;4;0;0\x1b\\");
        assert!(osc.update(&app).is_none());
        app.turn_error = Some("boom".into());
        assert_eq!(osc.update(&app).unwrap(), progress(2).into_bytes());
        crate::app::cleanup(app);
    }
}
