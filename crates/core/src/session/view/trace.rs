use crate::convo::record::Record;
use crate::prompt;
use crate::session::Session;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceItem {
    pub role: &'static str,
    pub summary: String,
    pub text: String,
}

fn role_of(tool: &str) -> &'static str {
    if tool == crate::session::turn::tool::TOOL {
        "lua"
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

use crate::convo::history::join_context;

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

impl Session {
    pub fn trace(&self) -> Vec<TraceItem> {
        let config = self.config();
        let st = self.state.lock().unwrap();
        let system = prompt::system_prompt(&config, st.bounds.drop > 0);
        let mut items = vec![item("system", system)];
        for r in &st.records {
            match r {
                Record::User {
                    text,
                    context,
                    images,
                    ..
                } => {
                    let extra = if images.is_empty() {
                        String::new()
                    } else {
                        format!("\n[{} image(s)]", images.len())
                    };
                    let body = join_context(text, context);
                    items.push(item("user", format!("{body}{extra}")));
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
                Record::Compaction { prune, drop } => items.push(item(
                    "compaction",
                    format!("pruned up to {prune}, dropped up to {drop}"),
                )),
                Record::Usage { model, usage, llm } => {
                    let src = if *llm { " (llm)" } else { "" };
                    items.push(item(
                        "usage",
                        format!(
                            "{model}{src}: input {} output {} cached {} cache_write {}",
                            usage.input, usage.output, usage.cached, usage.cache_write
                        ),
                    ));
                }
                Record::Settings { model, effort } => {
                    items.push(item("model", format!("{model} {effort}")))
                }
                Record::TurnEnd {
                    elapsed_ms,
                    calls,
                    model,
                    effort,
                    cancelled,
                } => {
                    let how = if *cancelled { "cancelled" } else { "done" };
                    items.push(item(
                        "turn",
                        format!("{how} in {elapsed_ms} ms, {calls} calls, {model} {effort}"),
                    ));
                }
                Record::Title { new } => items.push(item("title", new.clone())),
                Record::UiMarkdown {
                    name,
                    text,
                    pin,
                    removed,
                } => {
                    let body = if *removed {
                        format!("{name} removed")
                    } else {
                        format!("{name}{}\n\n{text}", if *pin { " (pinned)" } else { "" })
                    };
                    let mut it = item("show", body);
                    it.summary = format!("{name}{}", if *removed { " removed" } else { "" });
                    items.push(it);
                }
                Record::UiResult {
                    kind,
                    prompt,
                    value,
                    ..
                } => {
                    let role = if kind == "picker" { "pick" } else { "ask" };
                    let mut it = item(role, format!("{prompt}\n\n→ {value}"));
                    it.summary = format!("{} → {value}", first_line(prompt));
                    items.push(it);
                }
                Record::Job {
                    kind,
                    name,
                    code,
                    status,
                    ms,
                    error,
                } => {
                    let role = match kind.as_str() {
                        "task" => "task",
                        "command" => "command",
                        _ => "open",
                    };
                    let mut text = format!("{name} {status} ({ms} ms)");
                    if let Some(e) = error {
                        text.push_str(&format!("\n\n{e}"));
                    }
                    if !code.is_empty() {
                        text.push_str(&format!("\n\n{code}"));
                    }
                    let mut it = item(role, text);
                    it.summary = format!("{name} {status} ({ms} ms)");
                    items.push(it);
                }
                Record::Hook {
                    name,
                    event,
                    vars,
                    error,
                    ms,
                } => {
                    let result = error.as_deref().unwrap_or("ok");
                    let mut it = item(
                        "hook",
                        format!("{name} {event} {result} ({ms} ms)\n\n{vars}"),
                    );
                    it.summary = format!(
                        "{name} {event} {}",
                        if error.is_some() { "error" } else { "ok" }
                    );
                    items.push(it);
                }
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
