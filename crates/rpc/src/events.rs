use serde_json::{Value, json};
use tuse_core::Event;

pub fn to_json(e: &Event) -> Option<Value> {
    Some(match e {
        Event::Notification(t) => json!({"type": "notification", "text": t}),
        Event::TaskMessage(task) => {
            let mut v = serde_json::to_value(task).expect("task message is serializable");
            v["type"] = "task_message".into();
            v
        }
        Event::User { id, text, images } => {
            json!({"type": "user", "input_id": id.number(), "text": text, "images": images})
        }
        Event::TurnStart => json!({"type": "turn_start"}),
        Event::Text(t) => json!({"type": "text", "text": t}),
        Event::Reasoning(t) => json!({"type": "reasoning", "text": t}),
        Event::StreamReset => json!({"type": "stream_reset"}),
        Event::Assistant { text } => json!({"type": "assistant", "text": text}),
        Event::ToolDelta {
            index,
            id,
            name,
            code,
        } => {
            json!({"type": "tool_delta", "index": index, "id": id, "name": name, "code": code})
        }
        Event::ToolStart {
            timeout,
            id,
            tool,
            input,
        } => {
            json!({"type": "tool_start", "timeout": timeout, "id": id, "tool": tool, "input": input})
        }
        Event::ToolEnd {
            id,
            output,
            code,
            diffs,
        } => json!({"type": "tool_end", "id": id, "output": output, "code": code, "diffs": diffs}),
        Event::Status(t) => json!({"type": "status", "text": t}),
        Event::Notice(t) => json!({"type": "notice", "text": t}),
        Event::Flash(t) => json!({"type": "flash", "text": t}),
        Event::Error(t) => json!({"type": "error", "text": t}),
        Event::TurnEnd {
            elapsed,
            calls,
            model,
            effort,
            cancelled,
        } => json!({
            "type": "turn_end",
            "elapsed_ms": elapsed.as_millis() as u64,
            "calls": calls,
            "model": model,
            "effort": effort,
            "cancelled": cancelled,
        }),
        Event::Tasks => json!({"type": "tasks"}),
        Event::Usage => json!({"type": "usage"}),
        Event::Models => json!({"type": "models"}),
        Event::HistoryReplaced => json!({"type": "history_replaced"}),
        Event::Switch(_) => return None,
        Event::Reloaded => json!({"type": "reloaded"}),
        Event::UiMarkdown {
            name,
            text,
            pin,
            removed,
        } => {
            json!({"type": "ui_markdown", "name": name, "text": text, "pin": pin, "removed": removed})
        }
        Event::UiRequest(req) => {
            let mut v = serde_json::to_value(req).unwrap_or_default();
            if let Some(m) = v.as_object_mut() {
                m.insert("type".into(), json!("ui_request"));
            }
            v
        }
        Event::UiClear { id } => json!({"type": "ui_clear", "id": id}),
        Event::UiResult(entry) => {
            let mut v = serde_json::to_value(entry).unwrap_or_default();
            if let Some(m) = v.as_object_mut() {
                m.insert("type".into(), json!("ui_result"));
            }
            v
        }
    })
}
