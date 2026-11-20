use crate::prompt;
use crate::record::Record;
use crate::session::Session;

/// One entry of `/trace`.
pub struct TraceItem {
    /// `system`, `user`, `agent`, `thinking`, `python` (or `tool` for another name),
    /// `output`, `notice` or `error`.
    pub role: &'static str,
    /// The first meaningful line.
    pub summary: String,
    /// The whole entry, as shown when it is opened.
    pub text: String,
}

fn role_of(tool: &str) -> &'static str {
    if tool == "python" || crate::alias::is_builtin(tool) {
        "python"
    } else {
        "tool"
    }
}

fn item(role: &'static str, text: String) -> TraceItem {
    let summary = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    TraceItem {
        role,
        summary,
        text,
    }
}

impl Session {
    /// The whole session, one item per system prompt, message, thinking, tool call and output.
    pub fn trace(&self) -> Vec<TraceItem> {
        let config = self.config();
        let st = self.state.lock().unwrap();
        let system = prompt::system_prompt(&config, st.bounds.drop > 0);
        let mut items = vec![item("system", system)];
        for r in &st.records {
            match r {
                Record::User { text, images, .. } => {
                    let extra = if images.is_empty() {
                        String::new()
                    } else {
                        format!("\n[{} image(s)]", images.len())
                    };
                    items.push(item("user", format!("{text}{extra}")));
                }
                Record::Assistant {
                    text, reasoning, ..
                } => {
                    if !reasoning.trim().is_empty() {
                        items.push(item("thinking", reasoning.trim_end().to_string()));
                    }
                    if !text.trim().is_empty() {
                        items.push(item("agent", text.trim_end().to_string()));
                    }
                }
                Record::Tool {
                    tool,
                    input,
                    output,
                    code,
                    images,
                    diffs,
                    ..
                } => {
                    let code = code.map_or("none".to_string(), |c| c.to_string());
                    items.push(item(role_of(tool), input.clone()));
                    let mut out = item("output", output.trim_end().to_string());
                    let extra = if images.is_empty() {
                        String::new()
                    } else {
                        format!("\n[{} image(s)]", images.len())
                    };
                    let changes: String = diffs
                        .iter()
                        .map(|d| format!("\n\n{}\n{}", d.path, d.diff))
                        .collect();
                    out.text = format!("exit {code}\n\n{}{extra}{changes}", out.text);
                    if output.trim().is_empty() {
                        out.summary = format!("exit {code}");
                    }
                    items.push(out);
                }
                Record::Notification { text } => items.push(item("notification", text.clone())),
                Record::Notice { text } => items.push(item("notice", text.clone())),
                Record::Error { text } => items.push(item("error", text.clone())),
                _ => {}
            }
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use super::item;

    #[test]
    fn summary_is_the_first_nonempty_line() {
        let it = item("agent", "\n  \n hello\tworld \nmore".into());
        assert_eq!(it.summary, "hello world");
        assert!(it.text.contains("more"));
    }
}
