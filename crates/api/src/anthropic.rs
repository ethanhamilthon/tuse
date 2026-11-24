use crate::args::ArgStream;
use crate::client::{
    Client, check_status, finalize_calls, map_err, parse_data_url, parse_json, parse_json_object,
    read_body, read_sse, url_encode,
};
use crate::types::{ApiType, Delta, Error, Message, Replay, Reply, Request, ToolCall, Usage};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::AtomicBool;

const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u64 = 16384;
const MAX_MODEL_PAGES: usize = 10;

pub(crate) fn encode(req: &Request) -> Result<Value, Error> {
    let mut messages: Vec<Value> = vec![];
    for m in &req.messages {
        let (role, blocks) = match m {
            Message::User { text, images } => {
                let mut blocks = vec![];
                if !text.is_empty() {
                    blocks.push(json!({"type": "text", "text": text}));
                }
                for url in images {
                    let (mime, data) =
                        parse_data_url(url, &req.provider, ApiType::AnthropicMessages)?;
                    blocks.push(image_block(mime, data));
                }
                ("user", blocks)
            }
            Message::Assistant {
                text,
                tool_calls,
                replay,
            } => {
                let mut blocks = vec![];
                if let Some(r) = replay
                    .as_ref()
                    .filter(|r| r.matches(&req.provider, ApiType::AnthropicMessages, &req.model))
                {
                    blocks.extend(r.items.iter().cloned());
                } else {
                    if !text.is_empty() {
                        blocks.push(json!({"type": "text", "text": text}));
                    }
                    for c in tool_calls {
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": c.id,
                            "name": c.name,
                            "input": parse_json_object(&c.arguments),
                        }));
                    }
                }
                ("assistant", blocks)
            }
            Message::ToolResult {
                id, output, images, ..
            } => {
                let mut parts = vec![];
                if !output.is_empty() {
                    parts.push(json!({"type": "text", "text": output}));
                }
                if let Some(im) = images {
                    for url in &im.urls {
                        let (mime, data) =
                            parse_data_url(url, &req.provider, ApiType::AnthropicMessages)?;
                        parts.push(image_block(mime, data));
                    }
                }
                let content = if parts.is_empty() {
                    json!("")
                } else {
                    Value::Array(parts)
                };
                (
                    "user",
                    vec![json!({
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": content,
                    })],
                )
            }
        };
        if blocks.is_empty() {
            continue;
        }
        if let Some(last) = messages.last_mut()
            && last["role"] == role
            && let Some(arr) = last["content"].as_array_mut()
        {
            arr.extend(blocks);
        } else {
            messages.push(json!({"role": role, "content": blocks}));
        }
    }
    if let Some(last_user) = messages.iter_mut().rev().find(|m| m["role"] == "user")
        && let Some(last_block) = last_user["content"]
            .as_array_mut()
            .and_then(|a| a.last_mut())
        && let Some(obj) = last_block.as_object_mut()
    {
        obj.insert("cache_control".into(), json!({"type": "ephemeral"}));
    }
    let mut body = json!({
        "model": req.model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "messages": messages,
    });
    if !req.system.is_empty() {
        body["system"] = json!([{
            "type": "text",
            "text": req.system,
            "cache_control": {"type": "ephemeral"},
        }]);
    }
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect();
    }
    if let Some(effort) = &req.effort
        && !unfinished_loop_missing_thinking(req)
    {
        body["thinking"] = json!({"type": "adaptive"});
        body["output_config"] = json!({"effort": effort});
    }
    Ok(body)
}

fn image_block(mime: &str, data: &str) -> Value {
    json!({
        "type": "image",
        "source": {
            "type": "base64",
            "media_type": mime,
            "data": data,
        }
    })
}

fn unfinished_loop_missing_thinking(req: &Request) -> bool {
    let start = req
        .messages
        .iter()
        .rposition(|m| matches!(m, Message::User { .. }))
        .map_or(0, |i| i + 1);
    req.messages[start..].iter().any(|m| {
        let Message::Assistant {
            tool_calls, replay, ..
        } = m
        else {
            return false;
        };
        if tool_calls.is_empty() {
            return false;
        }
        !replay.as_ref().is_some_and(|r| {
            r.matches(&req.provider, ApiType::AnthropicMessages, &req.model)
                && r.items.first().is_some_and(|b| {
                    matches!(
                        b.get("type").and_then(Value::as_str),
                        Some("thinking" | "redacted_thinking")
                    )
                })
        })
    })
}

pub(crate) fn complete(
    client: &Client,
    req: &Request,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<Reply, Error> {
    let body = encode(req)?;
    let resp = check_status(
        client
            .agent
            .post(&format!("{}/messages", client.base_url))
            .set("x-api-key", &client.api_key)
            .set("anthropic-version", ANTHROPIC_VERSION)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(map_err)?,
    )?;
    let streaming = resp.content_type().contains("event-stream");
    let mut acc = Accumulator::new(&req.provider, &req.model);
    if !streaming {
        let v = parse_json(&read_body(resp)?)?;
        acc.fallback(&v, on_delta)?;
        return Ok(acc.finish());
    }
    read_sse(resp.into_reader(), cancel, |ev, data| {
        let v = parse_json(data)?;
        acc.event(ev, &v, on_delta)
    })?;
    if !acc.stopped || !acc.active.is_empty() {
        return Err(Error::Transport("stream ended before message_stop".into()));
    }
    Ok(acc.finish())
}

pub(crate) fn models(client: &Client) -> Result<Vec<String>, Error> {
    let mut out = vec![];
    let mut seen = HashSet::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_MODEL_PAGES {
        let url = match &after {
            Some(id) => format!(
                "{}/models?limit=100&after_id={}",
                client.base_url,
                url_encode(id)
            ),
            None => format!("{}/models?limit=100", client.base_url),
        };
        let resp = check_status(
            client
                .agent
                .get(&url)
                .set("x-api-key", &client.api_key)
                .set("anthropic-version", ANTHROPIC_VERSION)
                .call()
                .map_err(map_err)?,
        )?;
        let v = parse_json(&read_body(resp)?)?;
        out.extend(
            v["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| m["id"].as_str().map(String::from)),
        );
        let more = v.get("has_more").and_then(Value::as_bool) == Some(true);
        let next = v
            .get("last_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        match (more, next) {
            (true, Some(id)) if seen.insert(id.to_string()) => after = Some(id.to_string()),
            _ => break,
        }
    }
    Ok(out)
}

enum Block {
    Text(String),
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking(String),
    ToolUse {
        slot: usize,
        id: String,
        name: String,
    },
}

#[derive(Default)]
struct RawUsage {
    seen: bool,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
}

impl RawUsage {
    fn merge(&mut self, u: &Value) {
        self.seen = true;
        if let Some(n) = u.get("input_tokens").and_then(Value::as_u64) {
            self.input = n;
        }
        if let Some(n) = u.get("output_tokens").and_then(Value::as_u64) {
            self.output = n;
        }
        if let Some(n) = u.get("cache_read_input_tokens").and_then(Value::as_u64) {
            self.cache_read = n;
        }
        if let Some(n) = u.get("cache_creation_input_tokens").and_then(Value::as_u64) {
            self.cache_write = n;
        }
    }

    fn normalize(&self) -> Option<Usage> {
        self.seen.then_some(Usage {
            input: self.input + self.cache_read + self.cache_write,
            output: self.output,
            cached: self.cache_read,
            cache_write: self.cache_write,
        })
    }
}

struct Accumulator {
    provider: String,
    model: String,
    reply: Reply,
    usage: RawUsage,
    args: Vec<ArgStream>,
    active: BTreeMap<usize, Block>,
    finalized: BTreeMap<usize, Value>,
    stopped: bool,
}

impl Accumulator {
    fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            reply: Reply::default(),
            usage: RawUsage::default(),
            args: vec![],
            active: BTreeMap::new(),
            finalized: BTreeMap::new(),
            stopped: false,
        }
    }

    fn event(
        &mut self,
        ev: &str,
        v: &Value,
        on_delta: &mut dyn FnMut(Delta),
    ) -> Result<bool, Error> {
        let kind = v.get("type").and_then(Value::as_str).unwrap_or(ev);
        if kind == "error" {
            return Err(map_stream_error(v.get("error").unwrap_or(v)));
        }
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            return Err(map_stream_error(e));
        }
        match kind {
            "message_start" => {
                if let Some(u) = v
                    .get("message")
                    .and_then(|m| m.get("usage"))
                    .filter(|u| u.is_object())
                {
                    self.usage.merge(u);
                }
            }
            "content_block_start" => {
                let idx = v.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                if let Some(cb) = v.get("content_block") {
                    self.block_start(idx, cb, on_delta);
                }
            }
            "content_block_delta" => {
                let idx = v.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                if let Some(d) = v.get("delta") {
                    self.block_delta(idx, d, on_delta);
                }
            }
            "content_block_stop" => {
                let idx = v.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                if let Some(b) = self.active.remove(&idx) {
                    self.finalize_block(idx, b);
                }
            }
            "message_delta" => {
                if let Some(sr) = v
                    .get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(Value::as_str)
                {
                    self.reply.finish = sr.to_string();
                }
                if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
                    self.usage.merge(u);
                }
            }
            "message_stop" => {
                self.stopped = true;
                return Ok(false);
            }
            _ => {}
        }
        Ok(true)
    }

    fn block_start(&mut self, idx: usize, cb: &Value, on_delta: &mut dyn FnMut(Delta)) {
        match cb.get("type").and_then(Value::as_str) {
            Some("text") => {
                let t = cb.get("text").and_then(Value::as_str).unwrap_or("");
                if !t.is_empty() {
                    self.reply.text.push_str(t);
                    on_delta(Delta::Text(t));
                }
                self.active.insert(idx, Block::Text(t.to_string()));
            }
            Some("thinking") => {
                let t = cb.get("thinking").and_then(Value::as_str).unwrap_or("");
                let sig = cb.get("signature").and_then(Value::as_str).unwrap_or("");
                if !t.is_empty() {
                    self.reply.reasoning.push_str(t);
                    on_delta(Delta::Reasoning(t));
                }
                self.active.insert(
                    idx,
                    Block::Thinking {
                        thinking: t.to_string(),
                        signature: sig.to_string(),
                    },
                );
            }
            Some("redacted_thinking") => {
                let data = cb.get("data").and_then(Value::as_str).unwrap_or("");
                self.active
                    .insert(idx, Block::RedactedThinking(data.to_string()));
            }
            Some("tool_use") => {
                let id = cb
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = cb
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let slot = self.reply.tool_calls.len();
                let pre = cb
                    .get("input")
                    .filter(|v| v.as_object().is_some_and(|o| !o.is_empty()))
                    .map(Value::to_string)
                    .unwrap_or_default();
                let mut stream = ArgStream::default();
                let code = if pre.is_empty() {
                    String::new()
                } else {
                    stream.feed(&pre)
                };
                self.args.push(stream);
                self.reply.tool_calls.push(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: pre,
                });
                if !name.is_empty() {
                    on_delta(Delta::Tool {
                        index: slot,
                        id: &self.reply.tool_calls[slot].id,
                        name: &self.reply.tool_calls[slot].name,
                        code: &code,
                    });
                }
                self.active.insert(idx, Block::ToolUse { slot, id, name });
            }
            _ => {}
        }
    }

    fn block_delta(&mut self, idx: usize, d: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let Some(block) = self.active.get_mut(&idx) else {
            return;
        };
        match d.get("type").and_then(Value::as_str) {
            Some("text_delta") => {
                if let (Block::Text(buf), Some(t)) = (block, d.get("text").and_then(Value::as_str))
                {
                    buf.push_str(t);
                    self.reply.text.push_str(t);
                    on_delta(Delta::Text(t));
                }
            }
            Some("thinking_delta") => {
                if let (Block::Thinking { thinking, .. }, Some(t)) =
                    (block, d.get("thinking").and_then(Value::as_str))
                {
                    thinking.push_str(t);
                    self.reply.reasoning.push_str(t);
                    on_delta(Delta::Reasoning(t));
                }
            }
            Some("signature_delta") => {
                if let (Block::Thinking { signature, .. }, Some(s)) =
                    (block, d.get("signature").and_then(Value::as_str))
                {
                    signature.push_str(s);
                }
            }
            Some("input_json_delta") => {
                if let (Block::ToolUse { slot, .. }, Some(part)) =
                    (block, d.get("partial_json").and_then(Value::as_str))
                {
                    let slot = *slot;
                    let call = &mut self.reply.tool_calls[slot];
                    call.arguments.push_str(part);
                    let code = self.args[slot].feed(part);
                    if !call.name.is_empty() && !code.is_empty() {
                        on_delta(Delta::Tool {
                            index: slot,
                            id: &call.id,
                            name: &call.name,
                            code: &code,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn finalize_block(&mut self, idx: usize, block: Block) {
        let val = match block {
            Block::Text(t) if t.is_empty() => return,
            Block::Text(t) => json!({"type": "text", "text": t}),
            Block::Thinking {
                thinking,
                signature,
            } => json!({"type": "thinking", "thinking": thinking, "signature": signature}),
            Block::RedactedThinking(data) => json!({"type": "redacted_thinking", "data": data}),
            Block::ToolUse { slot, id, name } => {
                let call = &mut self.reply.tool_calls[slot];
                if call.arguments.is_empty() {
                    call.arguments = "{}".to_string();
                }
                json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": parse_json_object(&call.arguments),
                })
            }
        };
        self.finalized.insert(idx, val);
    }

    fn fallback(&mut self, v: &Value, on_delta: &mut dyn FnMut(Delta)) -> Result<(), Error> {
        if v.get("type").and_then(Value::as_str) == Some("error") {
            return Err(map_stream_error(v.get("error").unwrap_or(v)));
        }
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            return Err(map_stream_error(e));
        }
        if let Some(sr) = v.get("stop_reason").and_then(Value::as_str) {
            self.reply.finish = sr.to_string();
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            self.usage.merge(u);
        }
        for (idx, b) in v
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            match b.get("type").and_then(Value::as_str) {
                Some("thinking") => {
                    let t = b.get("thinking").and_then(Value::as_str).unwrap_or("");
                    if !t.is_empty() {
                        self.reply.reasoning.push_str(t);
                        on_delta(Delta::Reasoning(t));
                    }
                    self.finalized.insert(idx, b.clone());
                }
                Some("redacted_thinking") => {
                    self.finalized.insert(idx, b.clone());
                }
                Some("text") => {
                    let t = b.get("text").and_then(Value::as_str).unwrap_or("");
                    if !t.is_empty() {
                        self.reply.text.push_str(t);
                        on_delta(Delta::Text(t));
                        self.finalized.insert(idx, b.clone());
                    }
                }
                Some("tool_use") => {
                    let id = b
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let name = b
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let input = b.get("input").cloned().unwrap_or_else(|| json!({}));
                    let arguments = serde_json::to_string(&input).unwrap_or_else(|_| "{}".into());
                    let slot = self.reply.tool_calls.len();
                    let code = ArgStream::default().feed(&arguments);
                    self.reply.tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments,
                    });
                    let call = &self.reply.tool_calls[slot];
                    if !call.name.is_empty() {
                        on_delta(Delta::Tool {
                            index: slot,
                            id: &call.id,
                            name: &call.name,
                            code: &code,
                        });
                    }
                    self.finalized.insert(idx, b.clone());
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Reply {
        self.reply.usage = self.usage.normalize();
        finalize_calls(&mut self.reply.tool_calls);
        if !self.finalized.is_empty() {
            self.reply.replay = Some(Replay {
                provider: self.provider,
                api: ApiType::AnthropicMessages.as_str().to_string(),
                model: self.model,
                items: self.finalized.into_values().collect(),
            });
        }
        self.reply
    }
}

fn map_stream_error(e: &Value) -> Error {
    let code = match e.get("type").and_then(Value::as_str) {
        Some("rate_limit_error") => 429,
        Some("overloaded_error") => 529,
        Some("api_error") => 500,
        Some("invalid_request_error") => 400,
        Some("authentication_error") => 401,
        Some("permission_error") => 403,
        Some("not_found_error") => 404,
        Some("request_too_large") => 413,
        _ => 500,
    };
    Error::Status(code, e.to_string())
}
