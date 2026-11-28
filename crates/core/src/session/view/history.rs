use crate::convo::{record::Record, wire::join_context};
use serde::Serialize;
use serde_json::{Map, Value};
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

impl Record {
    pub fn role(&self) -> &'static str {
        match self {
            Record::System { .. } => "system",
            Record::User { .. } => "user",
            Record::Agent { .. } => "agent",
            Record::Lua { .. } => "lua",
            Record::Notification { .. } => "notification",
            Record::TaskMessage(_) => "task_message",
            Record::Message { .. } => "message",
            Record::Usage { .. } => "usage",
            Record::Meta { .. } => "meta",
            Record::TurnEnd { .. } => "turn_end",
            Record::Show { .. } => "show",
            Record::Pick { .. } => "pick",
            Record::Ask { .. } => "ask",
            Record::Task(_) => "task",
            Record::Command(_) => "command",
            Record::Open(_) => "open",
        }
    }

    fn detail(&self) -> String {
        match self {
            Record::System { text } | Record::Notification { text } => text.clone(),
            Record::TaskMessage(t) => t.text.clone(),
            Record::Message { text, .. } => text.clone(),
            Record::User {
                text,
                context,
                images,
            } => with_images(join_context(text, context), images.len()),
            Record::Agent {
                text,
                reasoning,
                cancelled,
                ..
            } => {
                let parts = [
                    text.as_str(),
                    reasoning.as_str(),
                    if *cancelled { "[cancelled]" } else { "" },
                ];
                parts
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
            Record::Lua {
                code,
                output,
                exit,
                images,
                diffs,
                ..
            } => {
                let exit = exit.map_or_else(|| "none".into(), |c| c.to_string());
                let mut detail = format!(
                    "{code}\n\nexit {exit}\n\n{}",
                    with_images(output.clone(), images.len())
                );
                for d in diffs {
                    detail.push_str(&format!("\n\n{}\n{}", d.path, d.diff));
                }
                detail
            }
            Record::Usage {
                model,
                input,
                output,
                cache_read,
                cache_write,
                llm,
            } => {
                let source = if *llm { " (llm)" } else { "" };
                format!(
                    "{model}{source}: input {input} output {output} cache_read {cache_read} cache_write {cache_write}"
                )
            }
            Record::Meta {
                title,
                model,
                effort,
            } => [title, model, effort]
                .into_iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
                .join(" "),
            Record::TurnEnd {
                elapsed_ms,
                calls,
                model,
                effort,
                cancelled,
            } => {
                let how = if *cancelled { "cancelled" } else { "done" };
                format!("{how} in {elapsed_ms} ms, {calls} calls, {model} {effort}")
            }
            Record::Show { name, text, pin } => {
                let marker = if text.is_empty() {
                    " removed"
                } else if *pin {
                    " (pinned)"
                } else {
                    ""
                };
                format!("{name}{marker}\n\n{text}")
            }
            Record::Pick { prompt, value, .. } | Record::Ask { prompt, value, .. } => {
                format!("{} → {value}\n\n{prompt}", first_line(prompt))
            }
            Record::Task(j) | Record::Command(j) | Record::Open(j) => format!(
                "{} {} ({} ms)\n\n{}\n\n{}",
                j.name,
                j.status,
                j.ms,
                j.error.as_deref().unwrap_or(""),
                j.code
            ),
        }
    }
}

impl From<&Record> for HistoryItem {
    fn from(r: &Record) -> Self {
        let detail = r.detail();
        let mut fields = serde_json::to_value(r)
            .unwrap()
            .as_object_mut()
            .map(std::mem::take)
            .unwrap();
        fields.remove("role");
        fields.retain(|_, v| !v.is_null());
        Self {
            role: r.role(),
            summary: first_line(&detail),
            fields,
            detail,
        }
    }
}

impl HistoryItem {
    pub fn is_empty_agent(&self) -> bool {
        self.role == "agent" && self.detail.is_empty()
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
        format!("{text}\n[{count} image(s)]")
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
            Record::User { .. }
            | Record::Agent { .. }
            | Record::TaskMessage(_)
            | Record::Notification { .. }
                if !pending.is_empty() =>
            {
                return Err(err("missing tool result before user or agent"));
            }
            Record::Agent { tool_calls, .. } => {
                for c in tool_calls {
                    if c.id.is_empty() || !pending.insert(c.id.clone()) {
                        return Err(err("empty or duplicate tool call id"));
                    }
                }
            }
            Record::Lua { id, .. } if !pending.remove(id) => {
                return Err(err("unexpected or duplicate tool result"));
            }
            _ => {}
        }
        if let Record::Lua { images, .. } | Record::User { images, .. } = &record
            && images
                .iter()
                .any(|u| !crate::convo::input::is_valid_image_data_url(u))
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
    match f.get("role") {
        None => return Err("missing role".into()),
        Some(Value::String(role)) if !tuse_config::HISTORY_ROLES.contains(&role.as_str()) => {
            return Err(format!("unknown history role {role}"));
        }
        Some(Value::String(_)) => {}
        Some(_) => return Err("role must be a string".into()),
    }
    serde_json::from_value(Value::Object(f)).map_err(|e| e.to_string())
}

pub(crate) fn keep_replay(records: &mut [Record], old: &[Record]) {
    for (i, record) in records.iter_mut().enumerate() {
        let Record::Agent {
            text,
            tool_calls,
            replay,
            ..
        } = record
        else {
            continue;
        };
        *replay = match old.get(i) {
            Some(Record::Agent {
                text: saved,
                tool_calls: calls,
                replay,
                ..
            }) if saved == text && calls == tool_calls => replay.clone(),
            _ => None,
        };
    }
}
