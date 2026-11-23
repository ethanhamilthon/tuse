//! Every `core::Event` becomes one `event` notification with a `type`.
use serde_json::{Value, json};
use tuse_core::Event;

pub fn to_json(e: &Event) -> Value {
    match e {
        Event::Notification(t) => json!({"type": "notification", "text": t}),
        Event::User { id, text, images } => {
            json!({"type": "user", "inputId": id.number(), "text": text, "images": images})
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
            "elapsedMs": elapsed.as_millis() as u64,
            "calls": calls,
            "model": model,
            "effort": effort,
            "cancelled": cancelled,
        }),
        Event::Tasks => json!({"type": "tasks"}),
        Event::Usage => json!({"type": "usage"}),
        Event::Models => json!({"type": "models"}),
    }
}
