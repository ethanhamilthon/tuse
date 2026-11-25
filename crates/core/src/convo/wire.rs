use crate::convo::record::Record;
use tuse_api::{Message, ToolImage};

pub fn messages(records: &[Record]) -> Vec<Message> {
    records.iter().filter_map(full).collect()
}

pub fn system(records: &[Record]) -> String {
    match records.first() {
        Some(Record::System { text }) => text.clone(),
        _ => String::new(),
    }
}

fn full(r: &Record) -> Option<Message> {
    match r {
        Record::Notification { text } => Some(Message::User {
            text: text.clone(),
            images: vec![],
        }),
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
            output: output.clone(),
            images: (!images.is_empty()).then(|| ToolImage {
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
        })
        .collect()
}
