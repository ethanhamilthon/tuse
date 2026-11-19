use crate::record::Record;
use serde_json::{Value, json};

/// History indices from the last compaction; see `compaction.rs`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Bounds {
    pub prune: usize,
    pub drop: usize,
}

pub const PRUNED_MARKER: &str = "\n[tool output pruned]";
pub const PRUNED_LINE_LIMIT: usize = 200;

/// Builds Chat Completions messages from records, applying compaction bounds.
pub fn messages(records: &[Record], b: Bounds, system: &str) -> Vec<Value> {
    let mut out = vec![json!({"role": "system", "content": system})];
    for (i, r) in records.iter().enumerate() {
        let msg = if i < b.drop {
            conversation_only(r)
        } else if i < b.prune {
            full(r, true)
        } else {
            full(r, false)
        };
        out.extend(msg);
    }
    out
}

fn conversation_only(r: &Record) -> Option<Value> {
    match r {
        Record::User { text, .. } => Some(json!({"role": "user", "content": text})),
        Record::Assistant { text, .. } if !text.trim().is_empty() => {
            Some(json!({"role": "assistant", "content": text}))
        }
        _ => None,
    }
}

fn full(r: &Record, prune: bool) -> Option<Value> {
    match r {
        Record::User { text, images, .. } => Some(user_message(text, images)),
        Record::Assistant {
            text, tool_calls, ..
        } => {
            let mut m = json!({"role": "assistant", "content": text});
            if !tool_calls.is_empty() {
                m["tool_calls"] = tool_calls
                    .iter()
                    .map(|c| json!({"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}))
                    .collect();
            }
            Some(m)
        }
        Record::Tool { id, output, .. } => {
            let content = if prune {
                prune_output(output)
            } else {
                output.clone()
            };
            Some(json!({"role": "tool", "tool_call_id": id, "content": content}))
        }
        _ => None,
    }
}

fn user_message(text: &str, images: &[String]) -> Value {
    if images.is_empty() {
        return json!({"role": "user", "content": text});
    }
    let mut parts = vec![json!({"type": "text", "text": text})];
    parts.extend(
        images
            .iter()
            .map(|url| json!({"type": "image_url", "image_url": {"url": url}})),
    );
    json!({"role": "user", "content": parts})
}

pub fn prune_output(output: &str) -> String {
    let (first, multiline) = match output.split_once('\n') {
        Some((f, _)) => (f, true),
        None => (output, false),
    };
    match first.char_indices().nth(PRUNED_LINE_LIMIT) {
        Some((i, _)) => format!("{}{PRUNED_MARKER}", &first[..i]),
        None if multiline => format!("{first}{PRUNED_MARKER}"),
        None => output.to_string(),
    }
}

/// Tool calls without a result (an interrupted step) get a placeholder result,
/// so every request stays valid. Returns the records to append.
pub fn missing_results(records: &[Record]) -> Vec<Record> {
    let Some(pos) = records
        .iter()
        .rposition(|r| matches!(r, Record::Assistant { tool_calls, .. } if !tool_calls.is_empty()))
    else {
        return vec![];
    };
    let Record::Assistant { tool_calls, .. } = &records[pos] else {
        return vec![];
    };
    tool_calls
        .iter()
        .filter(|c| {
            !records[pos..]
                .iter()
                .any(|r| matches!(r, Record::Tool { id, .. } if *id == c.id))
        })
        .map(|c| Record::Tool {
            id: c.id.clone(),
            command: crate::bash::command_of(&c.arguments),
            output: "[interrupted: no output]".into(),
            code: None,
            at: crate::id::now(),
        })
        .collect()
}

pub fn estimate_tokens(v: &[Value]) -> u64 {
    v.iter().map(|m| m.to_string().len() as u64 / 4).sum()
}
