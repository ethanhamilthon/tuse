use crate::args::ArgStream;
use crate::http::{
    Auth, Conn, active_turn_start, add_reasoning, add_text, check_err, drive_response,
    finalize_calls, jarr, jstr, ju64, make_replay, parse_data_url, parse_json_object, push_tool,
    push_turn, url_encode,
};
use crate::types::{ApiType, Delta, Error, Message, Reply, Request, Usage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

const ANTHROPIC_VERSION: &str = "2023-06-01";

fn push_images(out: &mut Vec<Value>, urls: &[String], prov: &str) -> Result<(), Error> {
    for u in urls {
        let (mime, data) = parse_data_url(u, prov, ApiType::AnthropicMessages)?;
        out.push(json!({"type": "image", "source": {"type": "base64", "media_type": mime, "data": data}}));
    }
    Ok(())
}

pub(crate) fn encode(req: &Request) -> Result<Value, Error> {
    let mut messages: Vec<Value> = vec![];
    for m in &req.messages {
        let (role, blocks) = match m {
            Message::User { text, images } => {
                let mut b = vec![];
                if !text.is_empty() {
                    b.push(json!({"type": "text", "text": text}));
                }
                push_images(&mut b, images, &req.provider)?;
                ("user", b)
            }
            Message::Assistant {
                text,
                tool_calls,
                replay,
            } => {
                let mut b = vec![];
                let hit = replay
                    .as_ref()
                    .filter(|r| r.matches(&req.provider, ApiType::AnthropicMessages, &req.model));
                if let Some(r) = hit {
                    b.extend(r.items.iter().cloned());
                } else {
                    if !text.is_empty() {
                        b.push(json!({"type": "text", "text": text}));
                    }
                    for c in tool_calls {
                        b.push(json!({"type": "tool_use", "id": c.id, "name": c.name, "input": parse_json_object(&c.arguments)}));
                    }
                }
                ("assistant", b)
            }
            Message::ToolResult {
                id, output, images, ..
            } => {
                let mut parts = vec![];
                if !output.is_empty() {
                    parts.push(json!({"type": "text", "text": output}));
                }
                if let Some(im) = images {
                    push_images(&mut parts, &im.urls, &req.provider)?;
                }
                let content = if parts.is_empty() {
                    json!("")
                } else {
                    Value::Array(parts)
                };
                (
                    "user",
                    vec![json!({"type": "tool_result", "tool_use_id": id, "content": content})],
                )
            }
        };
        push_turn(&mut messages, role, "content", blocks);
    }
    if let Some(obj) = messages
        .iter_mut()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|u| u["content"].as_array_mut())
        .and_then(|a| a.last_mut())
        .and_then(Value::as_object_mut)
    {
        obj.insert("cache_control".into(), json!({"type": "ephemeral"}));
    }
    let mut body =
        json!({"model": req.model, "max_tokens": 16384, "stream": true, "messages": messages});
    if !req.system.is_empty() {
        body["system"] =
            json!([{"type": "text", "text": req.system, "cache_control": {"type": "ephemeral"}}]);
    }
    if !req.tools.is_empty() {
        let ts: Vec<_> = req.tools.iter().map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.parameters})).collect();
        body["tools"] = Value::Array(ts);
    }
    if let Some(effort) = &req.effort
        && !missing_thinking(req)
    {
        body["thinking"] = json!({"type": "adaptive"});
        body["output_config"] = json!({"effort": effort});
    }
    Ok(body)
}

fn missing_thinking(req: &Request) -> bool {
    req.messages[active_turn_start(&req.messages)..]
        .iter()
        .any(|m| {
            let Message::Assistant {
                tool_calls, replay, ..
            } = m
            else {
                return false;
            };
            let ok = replay.as_ref().is_some_and(|r| {
                r.matches(&req.provider, ApiType::AnthropicMessages, &req.model)
                    && r.items.first().is_some_and(|b| {
                        matches!(jstr(b, "type"), Some("thinking" | "redacted_thinking"))
                    })
            });
            !tool_calls.is_empty() && !ok
        })
}

const AUTH: Auth = Auth::Header("x-api-key");

pub(crate) fn complete(
    conn: &Conn,
    req: &Request,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<Reply, Error> {
    let resp = conn.post_json(
        "/messages",
        &AUTH,
        &[("anthropic-version", ANTHROPIC_VERSION)],
        &encode(req)?,
    )?;
    let mut acc = Acc::default();
    let (streamed, _) = drive_response(resp, cancel, |ev, v| match ev {
        Some(ev) => acc.event(ev, &v, on_delta),
        None => acc.fallback(&v, on_delta),
    })?;
    acc.finish(streamed, req)
}

pub(crate) fn models(conn: &Conn) -> Result<Vec<String>, Error> {
    conn.paginate_models(
        &AUTH,
        &[("anthropic-version", ANTHROPIC_VERSION)],
        |cur| match cur {
            Some(id) => format!("/models?limit=100&after_id={}", url_encode(id)),
            None => "/models?limit=100".to_string(),
        },
        |v, out| {
            out.extend(
                jarr(v, "data")
                    .iter()
                    .filter_map(|m| jstr(m, "id").map(String::from)),
            );
            let more = v.get("has_more").and_then(Value::as_bool) == Some(true);
            more.then(|| jstr(v, "last_id").map(String::from)).flatten()
        },
    )
}

enum Block {
    Text(String),
    Thinking(String, String),
    Redacted(String),
    Tool(usize),
}

#[derive(Default)]
struct Acc {
    reply: Reply,
    usage_seen: bool,
    usage: [u64; 4],
    args: Vec<ArgStream>,
    active: BTreeMap<usize, Block>,
    finalized: BTreeMap<usize, Value>,
    stopped: bool,
}

impl Acc {
    fn finish(mut self, streamed: bool, req: &Request) -> Result<Reply, Error> {
        if streamed && (!self.stopped || !self.active.is_empty()) {
            return Err(Error::Transport("stream ended before message_stop".into()));
        }
        if self.usage_seen {
            let [inp, out, read, write] = self.usage;
            self.reply.usage = Some(Usage {
                input: inp + read + write,
                output: out,
                cache_read: read,
                cache_write: write,
            });
        }
        finalize_calls(&mut self.reply.tool_calls);
        let items = self.finalized.into_values().collect();
        self.reply.replay = make_replay(
            req.provider.clone(),
            ApiType::AnthropicMessages,
            req.model.clone(),
            items,
        );
        Ok(self.reply)
    }

    fn merge_usage(&mut self, u: &Value) {
        self.usage_seen = true;
        let keys = [
            "input_tokens",
            "output_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
        ];
        for (i, k) in keys.iter().enumerate() {
            if let Some(n) = ju64(u, k) {
                self.usage[i] = n;
            }
        }
    }

    fn fallback(&mut self, v: &Value, on_delta: &mut dyn FnMut(Delta)) -> Result<bool, Error> {
        check_err(v)?;
        if let Some(sr) = jstr(v, "stop_reason") {
            self.reply.finish = sr.into();
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            self.merge_usage(u);
        }
        for (i, b) in jarr(v, "content").iter().enumerate() {
            self.start(i, b, on_delta);
            if let Some(blk) = self.active.remove(&i) {
                self.stop(i, blk);
            }
        }
        Ok(true)
    }

    fn event(
        &mut self,
        ev: &str,
        v: &Value,
        on_delta: &mut dyn FnMut(Delta),
    ) -> Result<bool, Error> {
        check_err(v)?;
        let idx = ju64(v, "index").unwrap_or(0) as usize;
        match jstr(v, "type").unwrap_or(ev) {
            "message_start" => {
                if let Some(u) = v
                    .get("message")
                    .and_then(|m| m.get("usage"))
                    .filter(|u| u.is_object())
                {
                    self.merge_usage(u);
                }
            }
            "content_block_start" => {
                if let Some(cb) = v.get("content_block") {
                    self.start(idx, cb, on_delta);
                }
            }
            "content_block_delta" => {
                if let Some(d) = v.get("delta") {
                    self.delta(idx, d, on_delta);
                }
            }
            "content_block_stop" => {
                if let Some(b) = self.active.remove(&idx) {
                    self.stop(idx, b);
                }
            }
            "message_delta" => {
                if let Some(sr) = v.get("delta").and_then(|d| jstr(d, "stop_reason")) {
                    self.reply.finish = sr.into();
                }
                if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
                    self.merge_usage(u);
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

    fn start(&mut self, idx: usize, cb: &Value, on_delta: &mut dyn FnMut(Delta)) {
        match jstr(cb, "type") {
            Some("text") => {
                let t = jstr(cb, "text").unwrap_or("");
                add_text(&mut self.reply, t, on_delta);
                self.active.insert(idx, Block::Text(t.into()));
            }
            Some("thinking") => {
                let t = jstr(cb, "thinking").unwrap_or("");
                add_reasoning(&mut self.reply, t, on_delta);
                let sig = jstr(cb, "signature").unwrap_or("").into();
                self.active.insert(idx, Block::Thinking(t.into(), sig));
            }
            Some("redacted_thinking") => {
                self.active
                    .insert(idx, Block::Redacted(jstr(cb, "data").unwrap_or("").into()));
            }
            Some("tool_use") => {
                let pre = cb
                    .get("input")
                    .filter(|v| v.as_object().is_some_and(|o| !o.is_empty()))
                    .map(Value::to_string)
                    .unwrap_or_default();
                let id = jstr(cb, "id").unwrap_or("").into();
                let name = jstr(cb, "name").unwrap_or("").into();
                let slot = push_tool(
                    &mut self.reply.tool_calls,
                    Some(&mut self.args),
                    id,
                    name,
                    pre,
                    on_delta,
                );
                self.active.insert(idx, Block::Tool(slot));
            }
            _ => {}
        }
    }

    fn delta(&mut self, idx: usize, d: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let Some(blk) = self.active.get_mut(&idx) else {
            return;
        };
        match (jstr(d, "type"), blk) {
            (Some("text_delta"), Block::Text(buf)) if let Some(t) = jstr(d, "text") => {
                buf.push_str(t);
                add_text(&mut self.reply, t, on_delta);
            }
            (Some("thinking_delta"), Block::Thinking(buf, _))
                if let Some(t) = jstr(d, "thinking") =>
            {
                buf.push_str(t);
                add_reasoning(&mut self.reply, t, on_delta);
            }
            (Some("signature_delta"), Block::Thinking(_, sig))
                if let Some(s) = jstr(d, "signature") =>
            {
                sig.push_str(s);
            }
            (Some("input_json_delta"), Block::Tool(slot))
                if let Some(part) = jstr(d, "partial_json") =>
            {
                let (slot, c) = (*slot, &mut self.reply.tool_calls[*slot]);
                c.arguments.push_str(part);
                let code = self.args[slot].feed(part);
                if !c.name.is_empty() && !code.is_empty() {
                    on_delta(Delta::Tool {
                        index: slot,
                        id: &c.id,
                        name: &c.name,
                        code: &code,
                    });
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, idx: usize, block: Block) {
        let val = match block {
            Block::Text(t) if t.is_empty() => return,
            Block::Text(t) => json!({"type": "text", "text": t}),
            Block::Thinking(t, s) => json!({"type": "thinking", "thinking": t, "signature": s}),
            Block::Redacted(d) => json!({"type": "redacted_thinking", "data": d}),
            Block::Tool(slot) => {
                let c = &mut self.reply.tool_calls[slot];
                if c.arguments.is_empty() {
                    c.arguments = "{}".into();
                }
                json!({"type": "tool_use", "id": c.id, "name": c.name, "input": parse_json_object(&c.arguments)})
            }
        };
        self.finalized.insert(idx, val);
    }
}
