use crate::convo::record::Record;
use tuse_api::{Message, ToolImage};

pub fn messages(records: &[Record]) -> Vec<Message> {
    let mut names = std::collections::HashMap::new();
    for r in records {
        if let Record::Agent { tool_calls, .. } = r {
            names.extend(tool_calls.iter().map(|c| (c.id.as_str(), c.name.as_str())));
        }
    }
    records.iter().filter_map(|r| full(r, &names)).collect()
}

pub fn system(records: &[Record]) -> String {
    match records.first() {
        Some(Record::System { text }) => text.clone(),
        _ => String::new(),
    }
}

fn full(r: &Record, names: &std::collections::HashMap<&str, &str>) -> Option<Message> {
    Some(match r {
        Record::Notification { text } => Message::User {
            text: text.clone(),
            images: vec![],
        },
        Record::User {
            text,
            context,
            images,
            ..
        } => Message::User {
            text: join_context(text, context),
            images: images.clone(),
        },
        Record::Agent {
            text,
            tool_calls,
            replay,
            ..
        } => Message::Assistant {
            text: text.clone(),
            tool_calls: tool_calls.clone(),
            replay: replay.clone(),
        },
        Record::Lua {
            id,
            code,
            output,
            images,
            ..
        } => Message::ToolResult {
            id: id.clone(),
            name: names.get(id.as_str()).unwrap_or(&"lua").to_string(),
            output: output.clone(),
            images: (!images.is_empty()).then(|| ToolImage {
                label: code.clone(),
                urls: images.clone(),
            }),
        },
        _ => return None,
    })
}

pub(crate) fn join_context(text: &str, context: &str) -> String {
    match (text.is_empty(), context.is_empty()) {
        (_, true) => text.to_string(),
        (true, false) => context.to_string(),
        _ => format!("{text}\n\n{context}"),
    }
}

pub fn missing_results(records: &[Record]) -> Vec<Record> {
    let Some(pos) = records
        .iter()
        .rposition(|r| matches!(r, Record::Agent { tool_calls, .. } if !tool_calls.is_empty()))
    else {
        return vec![];
    };
    let Record::Agent { tool_calls, .. } = &records[pos] else {
        return vec![];
    };
    tool_calls
        .iter()
        .filter(|c| {
            !records[pos..]
                .iter()
                .any(|r| matches!(r, Record::Lua { id, .. } if *id == c.id))
        })
        .map(|c| Record::Lua {
            id: c.id.clone(),
            code: serde_json::from_str::<serde_json::Value>(&c.arguments)
                .ok()
                .and_then(|v| v["code"].as_str().map(String::from))
                .unwrap_or_else(|| c.arguments.clone()),
            output: "[interrupted: no output]".into(),
            exit: None,
            images: vec![],
            diffs: vec![],
        })
        .collect()
}
