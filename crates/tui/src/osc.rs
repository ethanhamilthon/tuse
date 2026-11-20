use crate::app::App;
use base64::{Engine, engine::general_purpose::STANDARD};

fn field(s: &str, limit: usize) -> String {
    let clean: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let s = clean.trim();
    let mut n = s.len().min(limit);
    while !s.is_char_boundary(n) {
        n -= 1
    }
    STANDARD.encode(&s[..n])
}
pub fn encode(state: &str, kind: Option<&str>, title: &str, msg: &str) -> String {
    let mut s = format!("\x1b]7501;state={state}:app=tuse");
    if let Some(k) = kind {
        s.push_str(":kind=");
        s.push_str(k);
    }
    for (key, v, max) in [("title", title, 192), ("msg", msg, 2048)] {
        let v = field(v, max);
        if !v.is_empty() {
            s.push_str(&format!(":{key}={v}"));
        }
    }
    s.push_str("\x1b\\");
    s
}
#[derive(Default)]
pub struct Osc {
    last: Option<String>,
}
impl Osc {
    fn changed(&mut self, s: String) -> Option<Vec<u8>> {
        if self.last.as_ref() == Some(&s) {
            return None;
        }
        let bytes = s.as_bytes().to_vec();
        self.last = Some(s);
        Some(bytes)
    }
    pub fn update(&mut self, app: &App) -> Option<Vec<u8>> {
        if !app.session.config().settings.ui.osc_status {
            self.last = None;
            return None;
        }
        let s = if let Some(q) = app
            .ask_input
            .as_ref()
            .map(|a| &a.1)
            .or_else(|| app.asks.first().map(|a| &a.1))
        {
            encode("blocked", Some("question"), "", q)
        } else if let Some((_, status)) = app.session.turn_info() {
            encode("working", None, &status, "")
        } else if let Some(err) = &app.turn_error {
            encode("error", None, "", err)
        } else if app.turn_done {
            encode("done", None, "", "")
        } else {
            encode("idle", None, "", "")
        };
        self.changed(s)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bytes_limits_and_dedup() {
        let working = encode("working", None, " A\nB ", "");
        assert_eq!(working, "\x1b]7501;state=working:app=tuse:title=QSBC\x1b\\");
        assert_eq!(
            encode("blocked", Some("question"), "", "?"),
            "\x1b]7501;state=blocked:app=tuse:kind=question:msg=Pw==\x1b\\"
        );
        let long = format!("{}é", "a".repeat(2047));
        assert_eq!(STANDARD.decode(field(&long, 2048)).unwrap().len(), 2047);
        let mut osc = Osc::default();
        assert!(osc.changed(working.clone()).is_some());
        assert!(osc.changed(working).is_none());
    }
}
