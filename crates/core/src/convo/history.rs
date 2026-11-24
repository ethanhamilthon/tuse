use crate::convo::record::Record;
use tuse_api::{Message, ToolImage};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Bounds {
    pub prune: usize,
    pub drop: usize,
}

pub const PRUNED_MARKER: &str = "\n[tool output pruned]";
pub const PRUNED_LINE_LIMIT: usize = 200;

pub fn messages(records: &[Record], b: Bounds) -> Vec<Message> {
    let mut out = vec![];
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

fn conversation_only(r: &Record) -> Option<Message> {
    match r {
        Record::User { text, .. } | Record::Notification { text } => Some(Message::User {
            text: text.clone(),
            images: vec![],
        }),
        Record::Assistant { text, .. } if !text.trim().is_empty() => Some(Message::Assistant {
            text: text.clone(),
            tool_calls: vec![],
            replay: None,
        }),
        _ => None,
    }
}

fn full(r: &Record, prune: bool) -> Option<Message> {
    match r {
        Record::Notification { .. } => conversation_only(r),
        Record::User {
            text,
            context,
            images,
            ..
        } => Some(Message::User {
            text: join_context(text, context),
            images: images.clone(),
        }),
        Record::Assistant {
            text,
            tool_calls,
            replay,
            ..
        } => Some(Message::Assistant {
            text: text.clone(),
            tool_calls: tool_calls.clone(),
            replay: replay.clone(),
        }),
        Record::Tool {
            id,
            tool,
            input,
            output,
            images,
            ..
        } => Some(Message::ToolResult {
            id: id.clone(),
            name: tool.clone(),
            output: if prune {
                prune_output(output)
            } else {
                output.clone()
            },
            images: (!prune && !images.is_empty()).then(|| ToolImage {
                label: input.clone(),
                urls: images.clone(),
            }),
        }),
        _ => None,
    }
}

pub(crate) fn join_context(text: &str, context: &str) -> String {
    match (text.is_empty(), context.is_empty()) {
        (_, true) => text.to_string(),
        (true, false) => context.to_string(),
        _ => format!("{text}\n\n{context}"),
    }
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

const IMAGE_TOKENS: u64 = 1000;

pub fn estimate_tokens(system: &str, msgs: &[Message]) -> u64 {
    (system.len() as u64 / 4) + msgs.iter().map(message_tokens).sum::<u64>()
}

fn message_tokens(m: &Message) -> u64 {
    match m {
        Message::User { text, images } => {
            (text.len() as u64 / 4) + images.len() as u64 * IMAGE_TOKENS
        }
        Message::Assistant {
            text, tool_calls, ..
        } => {
            let calls: usize = tool_calls
                .iter()
                .map(|c| c.id.len() + c.name.len() + c.arguments.len())
                .sum();
            (text.len() + calls) as u64 / 4
        }
        Message::ToolResult {
            id,
            name,
            output,
            images,
        } => {
            let img = images.as_ref().map_or(0, |im| im.urls.len() as u64) * IMAGE_TOKENS;
            ((id.len() + name.len() + output.len()) as u64 / 4) + img
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tuse_api::{Replay, ToolCall};

    #[test]
    fn notification_history_keeps_api_role() {
        let r = Record::Notification {
            text: "ready".into(),
        };
        assert_eq!(serde_json::to_value(&r).unwrap()["t"], "notification");
        assert!(matches!(
            full(&r, false),
            Some(Message::User { text, images }) if text == "ready" && images.is_empty()
        ));
    }

    fn user(t: &str, images: Vec<String>) -> Record {
        Record::User {
            text: t.into(),
            context: String::new(),
            images,
            at: String::new(),
        }
    }

    fn call(id: &str, replay: Option<Replay>) -> Record {
        Record::Assistant {
            text: String::new(),
            reasoning: String::new(),
            tool_calls: vec![ToolCall {
                id: id.into(),
                name: "lua".into(),
                arguments: r#"{"code":"read('a.png')"}"#.into(),
            }],
            replay,
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

    fn valid(msgs: &[Message]) -> bool {
        let mut open: Vec<&str> = vec![];
        for m in msgs {
            match m {
                Message::ToolResult { id, .. } => open.retain(|o| *o != id.as_str()),
                _ if !open.is_empty() => return false,
                _ => {}
            }
            if let Message::Assistant { tool_calls, .. } = m {
                open.extend(tool_calls.iter().map(|c| c.id.as_str()));
            }
        }
        open.is_empty()
    }

    #[test]
    fn messages_keep_tool_results_with_their_calls_and_respect_replay_zones() {
        let png = vec!["data:image/png;base64,AA".to_string()];
        let rep = Some(Replay {
            provider: "anth".into(),
            api: "anthropic-messages".into(),
            model: "claude".into(),
            items: vec![
                json!({"type": "thinking", "thinking": "x", "signature": "s".repeat(20000)}),
            ],
        });
        let records = vec![
            user("one", vec![]),
            call("c1", rep.clone()),
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
            call("c2", rep),
            result("c2", vec![]),
        ];
        for drop in 0..=records.len() {
            for prune in drop..=records.len() {
                let msgs = messages(&records, Bounds { prune, drop });
                assert!(valid(&msgs), "drop {drop} prune {prune}: {msgs:#?}");
            }
        }
        let msgs = messages(&records, Bounds { prune: 5, drop: 0 });
        assert!(matches!(
            &msgs[1],
            Message::Assistant {
                replay: Some(_),
                ..
            }
        ));
        assert!(matches!(&msgs[2], Message::ToolResult { images: None, .. }));
        assert!(estimate_tokens("", &msgs) < 3000, "opaque replay ignored");

        let dropped = messages(&records, Bounds { prune: 5, drop: 5 });
        assert!(matches!(&dropped[0], Message::User { text, .. } if text == "one"));
        assert!(matches!(&dropped[1], Message::User { text, .. } if text == "two"));
        assert!(matches!(
            &dropped[2],
            Message::Assistant {
                replay: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn a_crash_mid_step_gets_placeholder_results() {
        let mut records = vec![user("go", vec![]), call("c1", None)];
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
        assert!(valid(&messages(&records, Bounds::default())));
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
        assert!(matches!(full(&r, false), Some(Message::User { text, .. }) if text == both));
        assert!(
            matches!(conversation_only(&r), Some(Message::User { text, .. }) if text == "see #p")
        );
        assert_eq!(join_context("", "c"), "c");
    }

    #[test]
    fn old_user_and_assistant_records_remain_compatible() {
        let u: Record = serde_json::from_str(r#"{"t":"user","text":"hi","at":"x"}"#).unwrap();
        assert!(matches!(&u, Record::User { context, .. } if context.is_empty()));
        assert!(serde_json::to_value(&u).unwrap().get("context").is_none());

        let a: Record = serde_json::from_str(r#"{"t":"assistant","text":"ok","at":"x"}"#).unwrap();
        assert!(matches!(&a, Record::Assistant { replay: None, .. }));
        assert!(serde_json::to_value(&a).unwrap().get("replay").is_none());
    }
}
