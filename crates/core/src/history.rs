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
mod tests {
    use super::*;
    use tuse_api::ToolCall;

    #[test]
    fn notification_history_keeps_api_role() {
        let r = Record::Notification {
            text: "ready".into(),
        };
        assert_eq!(serde_json::to_value(&r).unwrap()["t"], "notification");
        assert_eq!(
            full(&r, false),
            Some(json!({"role":"user", "content":"ready"}))
        );
    }

    fn user(t: &str, images: Vec<String>) -> Record {
        Record::User {
            text: t.into(),
            context: String::new(),
            images,
            at: String::new(),
        }
    }

    fn call(id: &str) -> Record {
        Record::Assistant {
            text: String::new(),
            reasoning: String::new(),
            tool_calls: vec![ToolCall {
                id: id.into(),
                name: "lua".into(),
                arguments: r#"{"code":"read('a.png')"}"#.into(),
            }],
            at: String::new(),
        }
    }

    fn result(id: &str, images: Vec<String>) -> Record {
        Record::Tool {
            id: id.into(),
            tool: "lua".into(),
            input: "read('a.png')".into(),
            output: "ok".into(),
            code: Some(0),
            images,
            diffs: vec![],
            at: String::new(),
        }
    }

    /// Every `tool_call` is answered right after its message, before any `user` message.
    fn valid(msgs: &[Value]) -> bool {
        let mut open: Vec<String> = vec![];
        for m in msgs {
            match m["role"].as_str() {
                Some("tool") => open.retain(|id| m["tool_call_id"] != id.as_str()),
                _ if !open.is_empty() => return false,
                _ => {}
            }
            if let Some(calls) = m["tool_calls"].as_array() {
                open.extend(calls.iter().map(|c| c["id"].as_str().unwrap().to_string()));
            }
        }
        open.is_empty()
    }

    #[test]
    fn messages_keep_tool_results_with_their_calls() {
        let png = vec!["data:image/png;base64,AA".to_string()];
        let records = vec![
            user("one", vec![]),
            call("c1"),
            Record::Notice { text: "n".into() },
            result("c1", png.clone()),
            Record::Hook {
                name: "h".into(),
                event: "agent_message".into(),
                vars: String::new(),
                error: None,
                ms: 1,
            },
            user("two", png),
            call("c2"),
            result("c2", vec![]),
        ];
        for drop in 0..=records.len() {
            for prune in drop..=records.len() {
                let msgs = messages(&records, Bounds { prune, drop }, "s");
                assert!(valid(&msgs), "drop {drop} prune {prune}: {msgs:#?}");
            }
        }
        // The tool's image follows its result as a user message, before the next user text.
        let msgs = messages(&records, Bounds::default(), "s");
        let roles: Vec<_> = msgs.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(
            roles,
            [
                "system",
                "user",
                "assistant",
                "tool",
                "user",
                "user",
                "assistant",
                "tool"
            ]
        );
        assert_eq!(
            msgs[4]["content"][0]["text"],
            "[Images from the tool results above]"
        );
        assert_eq!(msgs[5]["content"][1]["type"], "image_url");
    }

    #[test]
    fn a_crash_mid_step_gets_placeholder_results() {
        let mut records = vec![user("go", vec![]), call("c1")];
        let Record::Assistant { tool_calls, .. } = &mut records[1] else {
            unreachable!()
        };
        tool_calls.push(ToolCall {
            id: "c2".into(),
            name: "lua".into(),
            arguments: "not json".into(),
        });
        records.push(result("c1", vec![]));
        let missing = missing_results(&records);
        assert_eq!(missing.len(), 1);
        let Record::Tool {
            id, input, code, ..
        } = &missing[0]
        else {
            panic!()
        };
        assert_eq!(
            (id.as_str(), input.as_str(), *code),
            ("c2", "not json", None)
        );
        records.extend(missing);
        assert!(missing_results(&records).is_empty());
        assert!(valid(&messages(&records, Bounds::default(), "s")));
    }

    #[test]
    fn context_rides_along_except_in_the_dropped_prefix() {
        let r = Record::User {
            text: "see #p".into(),
            context: "<prompt name=\"p\">body</prompt>".into(),
            images: vec![],
            at: String::new(),
        };
        let both = "see #p\n\n<prompt name=\"p\">body</prompt>";
        assert_eq!(full(&r, false).unwrap()["content"], both);
        assert_eq!(conversation_only(&r).unwrap()["content"], "see #p");
        assert_eq!(join_context("", "c"), "c");
    }

    #[test]
    fn old_user_records_have_no_context() {
        let r: Record = serde_json::from_str(r#"{"t":"user","text":"hi","at":"x"}"#).unwrap();
        assert!(matches!(&r, Record::User { context, .. } if context.is_empty()));
        assert!(serde_json::to_value(&r).unwrap().get("context").is_none());
    }
}
