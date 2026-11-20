use crate::history::{Bounds, estimate_tokens, messages};
use crate::record::Record;

/// Compaction keeps records intact and append-only; a `Compaction` record stores
/// two indices that every later request applies (see `history::messages`):
///
/// - before `drop` (the oldest zone) only user and assistant text remains;
/// - between `drop` and `prune` tool outputs are cut to their first line;
/// - from `prune` on (the freshest zone) everything is sent unchanged.
///
/// Measured from the end: the latest quarter of the threshold stays fresh, the
/// next quarter is pruned, everything older keeps only the conversation. `drop`
/// always lands on a user message, so tool calls and results go away together.
/// Bounds only move forward.
pub fn bounds(records: &[Record], threshold: u64, prev: Bounds) -> Option<Bounds> {
    let (fresh, middle) = (threshold / 4, threshold / 2);
    let mut next = prev;
    let mut total = 0;
    let mut prune_set = false;
    for i in (0..records.len()).rev() {
        let one = messages(&records[i..i + 1], Bounds::default(), "");
        if one.len() < 2 {
            continue;
        }
        total += estimate_tokens(&one[1..]);
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
    use tuse_api::ToolCall;

    fn turn(n: usize) -> Vec<Record> {
        let big = "x".repeat(4000);
        vec![
            Record::User {
                text: format!("q{n}"),
                images: vec![],
                at: String::new(),
            },
            Record::Assistant {
                text: String::new(),
                reasoning: String::new(),
                tool_calls: vec![ToolCall {
                    id: format!("c{n}"),
                    name: "python".into(),
                    arguments: "{}".into(),
                }],
                at: String::new(),
            },
            Record::Tool {
                id: format!("c{n}"),
                tool: "python".into(),
                input: "ls".into(),
                images: vec![],
                diffs: vec![],
                title: String::new(),
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
        let msgs = messages(&records, b, "s");
        let tools: Vec<_> = msgs.iter().filter(|m| m["role"] == "tool").collect();
        assert!(
            tools
                .iter()
                .any(|m| m["content"].as_str().unwrap().ends_with("pruned]"))
        );
        assert!(
            msgs.iter()
                .all(|m| m.get("tool_calls").is_none() || m["tool_calls"][0]["id"] != "c0")
        );
        assert_eq!(bounds(&records, 8000, b), None);
    }
}
