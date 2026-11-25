use crate::convo::{record::Record, wire::join_context};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
pub struct HistoryItem {
    pub role: &'static str,
    pub summary: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
    #[serde(skip)]
    pub detail: String,
}

impl From<&Record> for HistoryItem {
    fn from(r: &Record) -> Self {
        let (role, detail) = match r {
            Record::System { text } => ("system", text.clone()),
            Record::Flash { text } => ("flash", text.clone()),
            Record::User {
                text,
                context,
                images,
            } => (
                "user",
                with_images(join_context(text, context), images.len()),
            ),
            Record::Assistant {
                text,
                reasoning,
                tool_calls,
                cancelled,
                ..
            } => {
                let mut parts = vec![text.clone(), reasoning.clone()];
                parts.extend(
                    tool_calls
                        .iter()
                        .map(|c| format!("{}: {}", c.name, c.arguments)),
                );
                if *cancelled {
                    parts.push("[cancelled]".into());
                }
                (
                    "agent",
                    parts
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                )
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
                let role = if tool == "lua" { "lua" } else { "tool" };
                let code = code.map_or_else(|| "none".into(), |c| c.to_string());
                let mut detail = format!(
                    "{input}\n\nexit {code}\n\n{}",
                    with_images(output.clone(), images.len())
                );
                for d in diffs {
                    detail.push_str(&format!("\n\n{}\n{}", d.path, d.diff));
                }
                (role, detail)
            }
            Record::Notification { text } => ("notification", text.clone()),
            Record::Notice { text } => ("notice", text.clone()),
            Record::Error { text } => ("error", text.clone()),
            Record::Usage { model, usage, llm } => {
                let source = if *llm { " (llm)" } else { "" };
                (
                    "usage",
                    format!(
                        "{model}{source}: input {} output {} cached {} cache_write {}",
                        usage.input, usage.output, usage.cached, usage.cache_write
                    ),
                )
            }
            Record::Settings { model, effort } => ("model", format!("{model} {effort}")),
            Record::TurnEnd {
                elapsed_ms,
                calls,
                model,
                effort,
                cancelled,
            } => {
                let how = if *cancelled { "cancelled" } else { "done" };
                (
                    "turn",
                    format!("{how} in {elapsed_ms} ms, {calls} calls, {model} {effort}"),
                )
            }
            Record::Title { new } => ("title", new.clone()),
            Record::UiMarkdown {
                name,
                text,
                pin,
                removed,
            } => {
                let marker = if *removed {
                    " removed"
                } else if *pin {
                    " (pinned)"
                } else {
                    ""
                };
                ("show", format!("{name}{marker}\n\n{text}"))
            }
            Record::UiResult {
                kind,
                prompt,
                value,
                ..
            } => {
                let role = if kind == "picker" { "pick" } else { "ask" };
                (
                    role,
                    format!("{} → {value}\n\n{prompt}", first_line(prompt)),
                )
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
                (
                    role,
                    format!(
                        "{name} {status} ({ms} ms)\n\n{}\n\n{code}",
                        error.as_deref().unwrap_or("")
                    ),
                )
            }
            Record::Hook {
                name,
                event,
                vars,
                error,
                ms,
            } => {
                let result = error.as_deref().unwrap_or("ok");
                (
                    "hook",
                    format!("{name} {event} {result} ({ms} ms)\n\n{vars}"),
                )
            }
        };
        let mut fields = serde_json::to_value(r)
            .unwrap()
            .as_object_mut()
            .map(std::mem::take)
            .unwrap();
        fields.remove("t");
        match r {
            Record::Tool { .. } => rename(&mut fields, "tool", "name"),
            Record::Title { .. } => rename(&mut fields, "new", "text"),
            Record::Usage { .. } => {
                if let Some(Value::Object(usage)) = fields.remove("usage") {
                    fields.extend(usage);
                }
            }
            Record::Job { .. } | Record::UiResult { .. } => {
                fields.remove("kind");
            }
            _ => {}
        }
        fields.retain(|_, v| !v.is_null());
        Self {
            role,
            summary: first_line(&detail),
            fields,
            detail,
        }
    }
}

fn rename(fields: &mut Map<String, Value>, from: &str, to: &str) {
    if let Some(v) = fields.remove(from) {
        fields.insert(to.into(), v);
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .replace(char::is_control, " ")
}

fn with_images(text: String, count: usize) -> String {
    if count == 0 {
        text
    } else {
        format!("{text}\n[{} image(s)]", count)
    }
}

impl crate::Session {
    pub fn history(&self) -> Vec<HistoryItem> {
        self.with_records(|rs| rs.iter().map(HistoryItem::from).collect())
    }
}
#[derive(Default)]
pub(crate) struct Update {
    pub title: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub history: Option<Vec<Map<String, Value>>>,
}

pub(crate) fn decode(history: Vec<Map<String, Value>>) -> Result<Vec<Record>, String> {
    let mut records = Vec::with_capacity(history.len());
    let mut pending = HashSet::new();
    for (i, fields) in history.into_iter().enumerate() {
        let err = |s: &str| format!("history[{}]: {s}", i + 1);
        let record = decode_record(fields).map_err(|e| err(&e))?;
        match &record {
            Record::System { .. } if i != 0 => return Err(err("system must be first")),
            Record::User { .. } | Record::Assistant { .. } if !pending.is_empty() => {
                return Err(err("missing tool result before user or agent"));
            }
            Record::Assistant { tool_calls, .. } => {
                for c in tool_calls {
                    if c.id.is_empty() || !pending.insert(c.id.clone()) {
                        return Err(err("empty or duplicate tool call id"));
                    }
                }
            }
            Record::Tool { id, .. } if !pending.remove(id) => {
                return Err(err("unexpected or duplicate tool result"));
            }
            Record::Hook { vars, .. } if !vars.is_object() => {
                return Err(err("vars must be a table"));
            }
            _ => {}
        }
        if let Record::Tool { images, .. } | Record::User { images, .. } = &record
            && images.iter().any(|u| {
                !u.split_once(',').is_some_and(|(meta, data)| {
                    meta.strip_prefix("data:image/")
                        .is_some_and(|s| !s.is_empty())
                        && !data.is_empty()
                })
            })
        {
            return Err(err("images must be image data URLs"));
        }
        records.push(record);
    }
    if !pending.is_empty() {
        return Err("history: missing tool result at end of history".into());
    }
    Ok(records)
}

fn decode_record(mut f: Map<String, Value>) -> Result<Record, String> {
    f.remove("summary");
    let role = f.remove("role").ok_or("missing role")?;
    let t = match role.as_str().ok_or("role must be a string")? {
        "system" => "system",
        "user" => "user",
        "notification" => "notification",
        "agent" => "assistant",
        "lua" | "tool" => {
            let lua = role == "lua";
            if f.get("name")
                .and_then(Value::as_str)
                .is_none_or(|name| (name == "lua") != lua)
            {
                return Err("tool name does not match its role".into());
            }
            rename(&mut f, "name", "tool");
            "tool"
        }
        "notice" => "notice",
        "error" => "error",
        "flash" => "flash",
        "title" => {
            rename(&mut f, "text", "new");
            "title"
        }
        "usage" => {
            let usage: Map<String, Value> = ["input", "output", "cached", "cache_write"]
                .into_iter()
                .filter_map(|k| Some((k.into(), f.remove(k)?)))
                .collect();
            f.insert("usage".into(), Value::Object(usage));
            "usage"
        }
        "model" => "settings",
        "turn" => "turn_end",
        "hook" => "hook",
        "task" | "command" | "open" => {
            f.insert("kind".into(), role);
            "job"
        }
        "show" => "ui_markdown",
        "pick" | "ask" => {
            let kind = if role == "pick" { "picker" } else { "question" };
            f.insert("kind".into(), json!(kind));
            "ui_result"
        }
        _ => return Err(format!("unknown history role {role}")),
    };
    f.insert("t".into(), json!(t));
    serde_json::from_value(Value::Object(f)).map_err(|e| e.to_string())
}

pub(crate) fn keep_replay(records: &mut [Record], old: &[Record]) {
    for (i, record) in records.iter_mut().enumerate() {
        let Record::Assistant {
            text,
            tool_calls,
            replay,
            ..
        } = record
        else {
            continue;
        };
        *replay = match old.get(i) {
            Some(Record::Assistant {
                text: saved,
                tool_calls: calls,
                replay,
                ..
            }) if saved == text && calls == tool_calls => replay.clone(),
            _ => None,
        };
    }
}
