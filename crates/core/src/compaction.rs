use crate::history::{Bounds, estimate_tokens, messages};
use crate::record::Record;

pub fn bounds(records: &[Record], threshold: u64, prev: Bounds) -> Option<Bounds> {
    let (fresh, middle) = (threshold / 4, threshold / 2);
    let mut next = prev;
    let mut total = 0;
    let mut prune_set = false;
    for i in (0..records.len()).rev() {
        let one = messages(&records[i..i + 1], Bounds::default());
        if one.is_empty() {
            continue;
        }
        total += estimate_tokens("", &one);
        if !prune_set && total >= fresh {
            next.prune = prev.prune.max(i);
            prune_set = true;
        }
        if matches!(
            records[i],
            Record::User { .. } | Record::Notification { .. }
        ) && total >= middle
        {
            next.drop = prev.drop.max(i);
            break;
        }
    }
    (next != prev).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tuse_api::{Message, Replay, ToolCall};

    fn turn(n: usize) -> Vec<Record> {
        let big = "x".repeat(4000);
        vec![
            Record::User {
                text: format!("q{n}"),
                context: String::new(),
                images: vec![],
                at: String::new(),
            },
            Record::Assistant {
                text: String::new(),
                reasoning: String::new(),
                tool_calls: vec![ToolCall {
                    id: format!("c{n}"),
                    name: "lua".into(),
                    arguments: "{}".into(),
                }],
                replay: Some(Replay {
                    provider: "resp".into(),
                    api: "openai-responses".into(),
                    model: "m".into(),
                    items: vec![
                        json!({"type": "reasoning", "encrypted_content": "z".repeat(4000)}),
                    ],
                }),
                at: String::new(),
            },
            Record::Tool {
                id: format!("c{n}"),
                tool: "lua".into(),
                input: "ls".into(),
                images: vec![],
                diffs: vec![],
                output: format!("first\n{big}"),
                code: Some(0),
                at: String::new(),
            },
        ]
    }

    #[test]
    fn zones() {
        let records: Vec<Record> = (0..10).flat_map(turn).collect();
        let b = bounds(&records, 8000, Bounds::default()).unwrap();
        assert!(b.drop > 0 && b.drop <= b.prune && b.prune < records.len());
        assert!(matches!(records[b.drop], Record::User { .. }));
        let msgs = messages(&records, b);
        assert!(msgs.iter().any(|m| matches!(
            m,
            Message::ToolResult { output, .. } if output.ends_with("pruned]")
        )));
        assert!(msgs.iter().all(|m| !matches!(
            m,
            Message::Assistant { tool_calls, .. } if tool_calls.iter().any(|c| c.id == "c0")
        )));
        assert!(msgs.iter().any(|m| matches!(
            m,
            Message::Assistant {
                replay: Some(_),
                ..
            }
        )));
        assert_eq!(bounds(&records, 8000, b), None);
    }
}
