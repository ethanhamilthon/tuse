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
/// Images of tool results cannot ride in a `tool` message, so they follow the
/// results of that step as one `user` message.
pub fn messages(records: &[Record], b: Bounds, system: &str) -> Vec<Value> {
    let mut out = vec![json!({"role": "system", "content": system})];
    let mut images: Vec<(&str, &[String])> = vec![];
    for (i, r) in records.iter().enumerate() {
        // Records that are not messages (notices, usage, hooks) do not end a step.
        if matches!(
            r,
            Record::User { .. } | Record::Assistant { .. } | Record::Notification { .. }
        ) {
            flush_images(&mut out, &mut images);
        }
        let msg = if i < b.drop {
            conversation_only(r)
        } else if i < b.prune {
            full(r, true)
        } else {
            full(r, false)
        };
        if let Record::Tool {
            input, images: im, ..
        } = r
            && i >= b.prune
            && !im.is_empty()
        {
            images.push((input, im));
        }
        out.extend(msg);
    }
    flush_images(&mut out, &mut images);
    out
}

fn flush_images(out: &mut Vec<Value>, images: &mut Vec<(&str, &[String])>) {
    if images.is_empty() {
        return;
    }
    let mut parts = vec![json!({"type": "text", "text": "[Images from the tool results above]"})];
    for (path, urls) in images.drain(..) {
        parts.push(json!({"type": "text", "text": format!("[Image: {path}]")}));
        parts.extend(
            urls.iter()
                .map(|u| json!({"type": "image_url", "image_url": {"url": u}})),
        );
    }
    out.push(json!({"role": "user", "content": parts}));
}

fn conversation_only(r: &Record) -> Option<Value> {
    match r {
        Record::User { text, .. } | Record::Notification { text } => {
            Some(json!({"role": "user", "content": text}))
        }
        Record::Assistant { text, .. } if !text.trim().is_empty() => {
            Some(json!({"role": "assistant", "content": text}))
        }
        _ => None,
    }
}

fn full(r: &Record, prune: bool) -> Option<Value> {
    match r {
        Record::Notification { .. } => conversation_only(r),
        Record::User {
            text,
            context,
            images,
            ..
        } => Some(user_message(&join_context(text, context), images)),
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

/// The text and its hidden context as one message body.
pub(crate) fn join_context(text: &str, context: &str) -> String {
    match (text.is_empty(), context.is_empty()) {
        (_, true) => text.to_string(),
        (true, false) => context.to_string(),
        _ => format!("{text}\n\n{context}"),
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
            tool: c.name.clone(),
            input: serde_json::from_str::<serde_json::Value>(&c.arguments)
                .ok()
                .and_then(|v| v["code"].as_str().map(String::from))
                .unwrap_or_else(|| c.arguments.clone()),
            output: "[interrupted: no output]".into(),
            code: None,
            images: vec![],
            diffs: vec![],
            at: crate::id::now(),
        })
        .collect()
}

/// Tokens an image counts for in the estimate, whatever its size.
const IMAGE_TOKENS: u64 = 1000;

pub fn estimate_tokens(v: &[Value]) -> u64 {
    v.iter().map(message_tokens).sum()
}

fn message_tokens(m: &Value) -> u64 {
    let Some(parts) = m["content"].as_array() else {
        return m.to_string().len() as u64 / 4;
    };
    parts
        .iter()
        .map(|p| match p["type"].as_str() {
            Some("image_url") => IMAGE_TOKENS,
            _ => p.to_string().len() as u64 / 4,
        })
        .sum()
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
