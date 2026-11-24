use crate::args::ArgStream;
use crate::client::{
    Client, check_status, finalize_calls, map_err, openai_models, parse_json, read_body, read_sse,
};
use crate::types::{Delta, Error, Message, Reply, Request, ToolCall, ToolImage, Usage};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

pub(crate) fn encode(req: &Request) -> Value {
    let mut messages = vec![];
    if !req.system.is_empty() {
        messages.push(json!({"role": "system", "content": req.system}));
    }
    let mut images: Vec<&ToolImage> = vec![];
    for m in &req.messages {
        match m {
            Message::User { text, images: im } => {
                flush_images(&mut messages, &mut images);
                messages.push(user_message(text, im));
            }
            Message::Assistant {
                text, tool_calls, ..
            } => {
                flush_images(&mut messages, &mut images);
                let mut obj = json!({"role": "assistant", "content": text});
                if !tool_calls.is_empty() {
                    obj["tool_calls"] = tool_calls
                        .iter()
                        .map(|c| {
                            json!({
                                "id": c.id,
                                "type": "function",
                                "function": {"name": c.name, "arguments": c.arguments}
                            })
                        })
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
    flush_images(&mut messages, &mut images);
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
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect();
    }
    if let Some(e) = &req.effort {
        body["reasoning_effort"] = json!(e);
    }
    body
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

fn flush_images(out: &mut Vec<Value>, images: &mut Vec<&ToolImage>) {
    if images.is_empty() {
        return;
    }
    let mut parts = vec![json!({"type": "text", "text": "[Images from the tool results above]"})];
    for im in images.drain(..) {
        parts.push(json!({"type": "text", "text": format!("[Image: {}]", im.label)}));
        parts.extend(
            im.urls
                .iter()
                .map(|u| json!({"type": "image_url", "image_url": {"url": u}})),
        );
    }
    out.push(json!({"role": "user", "content": parts}));
}

pub(crate) fn complete(
    client: &Client,
    req: &Request,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<Reply, Error> {
    let body = encode(req);
    let resp = check_status(
        client
            .agent
            .post(&format!("{}/chat/completions", client.base_url))
            .set("Authorization", &format!("Bearer {}", client.api_key))
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(map_err)?,
    )?;
    let streaming = resp.content_type().contains("event-stream");
    let mut acc = Accumulator::default();
    if !streaming {
        let v = parse_json(&read_body(resp)?)?;
        check_error(&v)?;
        acc.chunk(&v, on_delta);
        return Ok(acc.finish());
    }
    let done = read_sse(resp.into_reader(), cancel, |_, data| {
        let v = parse_json(data)?;
        check_error(&v)?;
        acc.chunk(&v, on_delta);
        Ok(true)
    })?;
    if !done {
        return Err(Error::Transport("stream ended before [DONE]".into()));
    }
    Ok(acc.finish())
}

pub(crate) fn models(client: &Client) -> Result<Vec<String>, Error> {
    openai_models(client)
}

fn check_error(v: &Value) -> Result<(), Error> {
    match v.get("error") {
        Some(e) if !e.is_null() => Err(Error::Status(
            e.get("code").and_then(Value::as_u64).unwrap_or(500) as u16,
            e.to_string(),
        )),
        _ => Ok(()),
    }
}

#[derive(Default)]
struct Accumulator {
    reply: Reply,
    args: Vec<ArgStream>,
}

impl Accumulator {
    fn chunk(&mut self, v: &Value, on_delta: &mut dyn FnMut(Delta)) {
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            self.reply.usage = Some(usage(u));
        }
        let Some(choice) = v.get("choices").and_then(|c| c.get(0)) else {
            return;
        };
        if let Some(f) = choice.get("finish_reason").and_then(Value::as_str) {
            self.reply.finish = f.to_string();
        }
        let Some(delta) = choice.get("delta").or_else(|| choice.get("message")) else {
            return;
        };
        for key in ["reasoning_content", "reasoning"] {
            if let Some(t) = delta.get(key).and_then(Value::as_str) {
                self.reply.reasoning.push_str(t);
                on_delta(Delta::Reasoning(t));
            }
        }
        if let Some(t) = delta.get("content").and_then(Value::as_str) {
            self.reply.text.push_str(t);
            on_delta(Delta::Text(t));
        }
        for tc in delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.tool_call(tc, on_delta);
        }
    }

    fn tool_call(&mut self, tc: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let calls = &mut self.reply.tool_calls;
        let index = tc.get("index").and_then(Value::as_u64).map(|i| i as usize);
        let id = tc.get("id").and_then(Value::as_str).unwrap_or("");
        let slot = match index {
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
            if let Some(n) = f.get("name").and_then(Value::as_str) {
                call.name.push_str(n);
            }
            if let Some(a) = f.get("arguments").and_then(Value::as_str) {
                call.arguments.push_str(a);
            }
            if !call.name.is_empty() {
                let chunk = if first {
                    call.arguments.as_str()
                } else {
                    f.get("arguments").and_then(Value::as_str).unwrap_or("")
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

    fn finish(mut self) -> Reply {
        finalize_calls(&mut self.reply.tool_calls);
        self.reply
    }
}

fn usage(u: &Value) -> Usage {
    let n = |v: Option<&Value>| v.and_then(Value::as_u64).unwrap_or(0);
    let details = u.get("prompt_tokens_details");
    let d = |k: &str| n(details.and_then(|d| d.get(k)));
    Usage {
        input: n(u.get("prompt_tokens")),
        output: n(u.get("completion_tokens")),
        cached: d("cached_tokens"),
        cache_write: d("cache_write_tokens").max(d("cached_creation_tokens")),
    }
}
