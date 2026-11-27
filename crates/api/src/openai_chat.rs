use crate::args::ArgStream;
use crate::http::{
    Auth, Conn, add_reasoning, add_text, check_err, drive_response, finalize_calls,
    flush_tool_images, jarr, jstr, ju64,
};
use crate::types::{Delta, Error, Message, Reply, Request, ToolCall, ToolImage, Usage};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn img_part(url: &str) -> Value {
    json!({"type": "image_url", "image_url": {"url": url}})
}

pub(crate) fn encode(req: &Request) -> Value {
    let mut messages = vec![];
    if !req.system.is_empty() {
        messages.push(json!({"role": "system", "content": req.system}));
    }
    let mut images: Vec<&ToolImage> = vec![];
    for m in &req.messages {
        match m {
            Message::User { text, images: im } => {
                flush_tool_images(&mut messages, &mut images, "text", img_part);
                if im.is_empty() {
                    messages.push(json!({"role": "user", "content": text}));
                } else {
                    let mut parts = vec![json!({"type": "text", "text": text})];
                    parts.extend(im.iter().map(|u| img_part(u)));
                    messages.push(json!({"role": "user", "content": parts}));
                }
            }
            Message::Assistant {
                text, tool_calls, ..
            } => {
                flush_tool_images(&mut messages, &mut images, "text", img_part);
                let mut obj = json!({"role": "assistant", "content": text});
                if !tool_calls.is_empty() {
                    obj["tool_calls"] = tool_calls
                        .iter()
                        .map(|c| json!({"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}))
                        .collect();
                }
                messages.push(obj);
            }
            Message::ToolResult {
                id,
                output,
                images: im,
                ..
            } => {
                messages.push(json!({"role": "tool", "tool_call_id": id, "content": output}));
                if let Some(im) = im.as_ref().filter(|im| !im.urls.is_empty()) {
                    images.push(im);
                }
            }
        }
    }
    flush_tool_images(&mut messages, &mut images, "text", img_part);
    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "stream": true,
        "stream_options": {"include_usage": true},
    });
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.parameters}}))
            .collect();
    }
    if let Some(e) = &req.effort {
        body["reasoning_effort"] = json!(e);
    }
    body
}

pub(crate) fn complete(
    conn: &Conn,
    req: &Request,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<Reply, Error> {
    let resp = conn.post_json("/chat/completions", &Auth::Bearer, &[], &encode(req))?;
    let mut acc = Accumulator::default();
    let (streamed, done) = drive_response(resp, cancel, |_, v| {
        check_err(&v)?;
        acc.chunk(&v, on_delta);
        Ok(true)
    })?;
    acc.finish(streamed && !done)
}

#[derive(Default)]
struct Accumulator {
    reply: Reply,
    args: Vec<ArgStream>,
}

impl Accumulator {
    fn finish(mut self, truncated: bool) -> Result<Reply, Error> {
        if truncated {
            return Err(Error::Transport("stream ended before [DONE]".into()));
        }
        finalize_calls(&mut self.reply.tool_calls);
        Ok(self.reply)
    }

    fn chunk(&mut self, v: &Value, on_delta: &mut dyn FnMut(Delta)) {
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            let d = |k| {
                u.get("prompt_tokens_details")
                    .and_then(|d| ju64(d, k))
                    .unwrap_or(0)
            };
            self.reply.usage = Some(Usage {
                input: ju64(u, "prompt_tokens").unwrap_or(0),
                output: ju64(u, "completion_tokens").unwrap_or(0),
                cache_read: d("cached_tokens"),
                cache_write: d("cache_write_tokens").max(d("cached_creation_tokens")),
            });
        }
        let Some(choice) = jarr(v, "choices").first() else {
            return;
        };
        if let Some(f) = jstr(choice, "finish_reason") {
            self.reply.finish = f.to_string();
        }
        let Some(delta) = choice.get("delta").or_else(|| choice.get("message")) else {
            return;
        };
        for k in ["reasoning_content", "reasoning"] {
            if let Some(t) = jstr(delta, k) {
                add_reasoning(&mut self.reply, t, on_delta);
            }
        }
        if let Some(t) = jstr(delta, "content") {
            add_text(&mut self.reply, t, on_delta);
        }
        for tc in jarr(delta, "tool_calls") {
            self.tool_call(tc, on_delta);
        }
    }

    fn tool_call(&mut self, tc: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let calls = &mut self.reply.tool_calls;
        let id = jstr(tc, "id").unwrap_or("");
        let slot = match ju64(tc, "index").map(|i| i as usize) {
            Some(i) => i,
            None if !id.is_empty() => calls.len(),
            None => calls.len().saturating_sub(1),
        };
        while calls.len() <= slot {
            self.args.push(Default::default());
            calls.push(ToolCall::default());
        }
        let call = &mut calls[slot];
        if !id.is_empty() {
            call.id = id.to_string();
        }
        let first = call.name.is_empty();
        if let Some(f) = tc.get("function") {
            if let Some(n) = jstr(f, "name") {
                call.name.push_str(n);
            }
            if let Some(a) = jstr(f, "arguments") {
                call.arguments.push_str(a);
            }
            if !call.name.is_empty() {
                let chunk = if first {
                    &call.arguments
                } else {
                    jstr(f, "arguments").unwrap_or("")
                };
                let code = self.args[slot].feed(chunk);
                if first || !code.is_empty() {
                    on_delta(Delta::Tool {
                        index: slot,
                        id: &call.id,
                        name: &call.name,
                        code: &code,
                    });
                }
            }
        }
    }
}
