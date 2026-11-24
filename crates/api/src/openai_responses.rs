use crate::args::ArgStream;
use crate::client::{
    Client, check_status, finalize_calls, map_err, openai_models, parse_json, read_body, read_sse,
};
use crate::types::{
    ApiType, Delta, Error, Message, Replay, Reply, Request, ToolCall, ToolImage, Usage,
};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

pub(crate) fn encode(req: &Request) -> Value {
    let mut input = vec![];
    let mut images: Vec<&ToolImage> = vec![];
    for m in &req.messages {
        match m {
            Message::User { text, images: im } => {
                flush_images(&mut input, &mut images);
                let mut parts = vec![json!({"type": "input_text", "text": text})];
                parts.extend(
                    im.iter()
                        .map(|url| json!({"type": "input_image", "image_url": url})),
                );
                input.push(json!({"role": "user", "content": parts}));
            }
            Message::Assistant {
                text,
                tool_calls,
                replay,
            } => {
                flush_images(&mut input, &mut images);
                if let Some(r) = replay
                    .as_ref()
                    .filter(|r| r.matches(&req.provider, ApiType::OpenAiResponses, &req.model))
                {
                    input.extend(r.items.iter().cloned());
                } else {
                    if !text.is_empty() {
                        input.push(json!({
                            "role": "assistant",
                            "content": [{"type": "output_text", "text": text}]
                        }));
                    }
                    for c in tool_calls {
                        input.push(json!({
                            "type": "function_call",
                            "call_id": c.id,
                            "name": c.name,
                            "arguments": c.arguments,
                        }));
                    }
                }
            }
            Message::ToolResult {
                id,
                output,
                images: im,
                ..
            } => {
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": id,
                    "output": output,
                }));
                if let Some(im) = im.as_ref().filter(|im| !im.urls.is_empty()) {
                    images.push(im);
                }
            }
        }
    }
    flush_images(&mut input, &mut images);
    let mut body = json!({
        "model": req.model,
        "input": input,
        "stream": true,
        "store": false,
    });
    if !req.system.is_empty() {
        body["instructions"] = json!(req.system);
    }
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            })
            .collect();
    }
    if let Some(e) = &req.effort {
        body["reasoning"] = json!({"effort": e, "summary": "auto"});
        body["include"] = json!(["reasoning.encrypted_content"]);
    }
    body
}

fn flush_images(out: &mut Vec<Value>, images: &mut Vec<&ToolImage>) {
    if images.is_empty() {
        return;
    }
    let mut parts =
        vec![json!({"type": "input_text", "text": "[Images from the tool results above]"})];
    for im in images.drain(..) {
        parts.push(json!({"type": "input_text", "text": format!("[Image: {}]", im.label)}));
        parts.extend(
            im.urls
                .iter()
                .map(|u| json!({"type": "input_image", "image_url": u})),
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
            .post(&format!("{}/responses", client.base_url))
            .set("Authorization", &format!("Bearer {}", client.api_key))
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
        acc.event(ev, &v, on_delta)?;
        Ok(true)
    })?;
    if !acc.completed {
        return Err(Error::Transport(
            "stream ended before response.completed".into(),
        ));
    }
    Ok(acc.finish())
}

pub(crate) fn models(client: &Client) -> Result<Vec<String>, Error> {
    openai_models(client)
}

#[derive(Default)]
struct Slot {
    output_index: Option<usize>,
    item_id: String,
    call: ToolCall,
    args: ArgStream,
    streamed: bool,
    announced: bool,
}

struct Accumulator {
    provider: String,
    model: String,
    reply: Reply,
    slots: Vec<Slot>,
    replay: Vec<(usize, Value)>,
    completed: bool,
}

impl Accumulator {
    fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            reply: Reply::default(),
            slots: vec![],
            replay: vec![],
            completed: false,
        }
    }

    fn slot(&mut self, item_id: &str, output_index: Option<usize>) -> usize {
        if !item_id.is_empty()
            && let Some(i) = self.slots.iter().position(|s| s.item_id == item_id)
        {
            return i;
        }
        if let Some(oi) = output_index
            && let Some(i) = self.slots.iter().position(|s| s.output_index == Some(oi))
        {
            return i;
        }
        self.slots.push(Slot {
            output_index,
            item_id: item_id.to_string(),
            ..Default::default()
        });
        self.slots.len() - 1
    }

    fn event(&mut self, ev: &str, v: &Value, on_delta: &mut dyn FnMut(Delta)) -> Result<(), Error> {
        let kind = v.get("type").and_then(Value::as_str).unwrap_or(ev);
        if kind == "error" {
            return Err(map_stream_error(v.get("error").unwrap_or(v)));
        }
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            return Err(map_stream_error(e));
        }
        match kind {
            "response.output_text.delta" => {
                if let Some(t) = v.get("delta").and_then(Value::as_str) {
                    self.reply.text.push_str(t);
                    on_delta(Delta::Text(t));
                }
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                if let Some(t) = v.get("delta").and_then(Value::as_str) {
                    self.reply.reasoning.push_str(t);
                    on_delta(Delta::Reasoning(t));
                }
            }
            "response.output_item.added" => {
                let oi = v
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .map(|n| n as usize);
                if let Some(item) = v.get("item")
                    && item.get("type").and_then(Value::as_str) == Some("function_call")
                {
                    self.item_added(oi, item, on_delta);
                }
            }
            "response.function_call_arguments.delta" => {
                let item_id = v.get("item_id").and_then(Value::as_str).unwrap_or("");
                let oi = v
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .map(|n| n as usize);
                let delta = v.get("delta").and_then(Value::as_str).unwrap_or("");
                let idx = self.slot(item_id, oi);
                let s = &mut self.slots[idx];
                if !delta.is_empty() {
                    s.call.arguments.push_str(delta);
                    s.streamed = true;
                }
                let code = s.args.feed(delta);
                if !s.call.name.is_empty() && (!s.announced || !code.is_empty()) {
                    s.announced = true;
                    on_delta(Delta::Tool {
                        index: idx,
                        id: &s.call.id,
                        name: &s.call.name,
                        code: &code,
                    });
                }
            }
            "response.function_call_arguments.done" => {
                let item_id = v.get("item_id").and_then(Value::as_str).unwrap_or("");
                let oi = v
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .map(|n| n as usize);
                let idx = self.slot(item_id, oi);
                self.finalize_args(
                    idx,
                    v.get("call_id").and_then(Value::as_str),
                    (!item_id.is_empty()).then_some(item_id),
                    v.get("name").and_then(Value::as_str),
                    v.get("arguments").and_then(Value::as_str),
                    on_delta,
                );
            }
            "response.output_item.done" => {
                let oi = v
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .map(|n| n as usize);
                if let Some(item) = v.get("item") {
                    self.item_done(oi, item, on_delta);
                }
            }
            "response.completed" | "response.incomplete" => {
                self.completed = true;
                if let Some(r) = v.get("response") {
                    if let Some(st) = r.get("status").and_then(Value::as_str) {
                        self.reply.finish = st.to_string();
                    }
                    if let Some(u) = r.get("usage").filter(|u| u.is_object()) {
                        self.reply.usage = Some(usage(u));
                    }
                }
            }
            "response.failed" => {
                let err = v
                    .get("response")
                    .and_then(|r| r.get("error"))
                    .filter(|e| !e.is_null())
                    .unwrap_or(v);
                return Err(map_stream_error(err));
            }
            _ => {}
        }
        Ok(())
    }

    fn item_added(&mut self, oi: Option<usize>, item: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let item_id = item.get("id").and_then(Value::as_str).unwrap_or("");
        let call_id = item
            .get("call_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(item_id);
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        let args = item.get("arguments").and_then(Value::as_str).unwrap_or("");
        let idx = self.slot(item_id, oi);
        let s = &mut self.slots[idx];
        if !item_id.is_empty() {
            s.item_id = item_id.to_string();
        }
        if oi.is_some() {
            s.output_index = oi;
        }
        if !call_id.is_empty() {
            s.call.id = call_id.to_string();
        }
        if !name.is_empty() {
            s.call.name = name.to_string();
        }
        let code = if !args.is_empty() {
            s.call.arguments.push_str(args);
            s.streamed = true;
            s.args.feed(args)
        } else {
            String::new()
        };
        if !s.call.name.is_empty() && (!s.announced || !code.is_empty()) {
            s.announced = true;
            on_delta(Delta::Tool {
                index: idx,
                id: &s.call.id,
                name: &s.call.name,
                code: &code,
            });
        }
    }

    fn item_done(&mut self, oi: Option<usize>, item: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let order = oi.unwrap_or(self.replay.len());
        match item.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                let item_id = item.get("id").and_then(Value::as_str).unwrap_or("");
                let idx = self.slot(item_id, oi);
                if !item_id.is_empty() {
                    self.slots[idx].item_id = item_id.to_string();
                }
                self.finalize_args(
                    idx,
                    item.get("call_id").and_then(Value::as_str),
                    (!item_id.is_empty()).then_some(item_id),
                    item.get("name").and_then(Value::as_str),
                    item.get("arguments").and_then(Value::as_str),
                    on_delta,
                );
                let mut synced = item.clone();
                if let Some(obj) = synced.as_object_mut() {
                    let c = &self.slots[idx].call;
                    if !c.id.is_empty() {
                        obj.insert("call_id".into(), json!(c.id));
                    }
                    if !c.name.is_empty() {
                        obj.insert("name".into(), json!(c.name));
                    }
                    obj.insert("arguments".into(), json!(c.arguments));
                }
                self.replay.push((order, synced));
            }
            Some("reasoning") => {
                if has_encrypted_content(item) {
                    self.replay.push((order, item.clone()));
                }
            }
            Some("message") => self.replay.push((order, item.clone())),
            _ => {}
        }
    }

    fn finalize_args(
        &mut self,
        idx: usize,
        explicit_call_id: Option<&str>,
        fallback_id: Option<&str>,
        name: Option<&str>,
        arguments: Option<&str>,
        on_delta: &mut dyn FnMut(Delta),
    ) {
        let s = &mut self.slots[idx];
        if let Some(cid) = explicit_call_id.filter(|c| !c.is_empty()) {
            s.call.id = cid.to_string();
        } else if s.call.id.is_empty()
            && let Some(fid) = fallback_id.filter(|c| !c.is_empty())
        {
            s.call.id = fid.to_string();
        }
        if let Some(n) = name.filter(|n| !n.is_empty())
            && s.call.name.is_empty()
        {
            s.call.name = n.to_string();
        }
        if let Some(a) = arguments.filter(|a| !a.is_empty()) {
            s.call.arguments = a.to_string();
            if !s.streamed {
                s.streamed = true;
                let code = s.args.feed(a);
                if !s.call.name.is_empty() && (!s.announced || !code.is_empty()) {
                    s.announced = true;
                    on_delta(Delta::Tool {
                        index: idx,
                        id: &s.call.id,
                        name: &s.call.name,
                        code: &code,
                    });
                }
            }
        }
    }

    fn fallback(&mut self, v: &Value, on_delta: &mut dyn FnMut(Delta)) -> Result<(), Error> {
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            return Err(map_stream_error(e));
        }
        if v.get("status").and_then(Value::as_str) == Some("failed") {
            return Err(map_stream_error(v));
        }
        if let Some(st) = v.get("status").and_then(Value::as_str) {
            self.reply.finish = st.to_string();
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            self.reply.usage = Some(usage(u));
        }
        for (i, item) in v
            .get("output")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            match item.get("type").and_then(Value::as_str) {
                Some("reasoning") => {
                    for s in item
                        .get("summary")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(t) = s.get("text").and_then(Value::as_str) {
                            self.reply.reasoning.push_str(t);
                            on_delta(Delta::Reasoning(t));
                        }
                    }
                    if has_encrypted_content(item) {
                        self.replay.push((i, item.clone()));
                    }
                }
                Some("message") => {
                    for c in item
                        .get("content")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if c.get("type").and_then(Value::as_str) == Some("output_text")
                            && let Some(t) = c.get("text").and_then(Value::as_str)
                        {
                            self.reply.text.push_str(t);
                            on_delta(Delta::Text(t));
                        }
                    }
                    self.replay.push((i, item.clone()));
                }
                Some("function_call") => self.item_done(Some(i), item, on_delta),
                _ => {}
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Reply {
        self.reply.tool_calls = self.slots.into_iter().map(|s| s.call).collect();
        finalize_calls(&mut self.reply.tool_calls);
        if !self.replay.is_empty() {
            self.replay.sort_by_key(|(i, _)| *i);
            self.reply.replay = Some(Replay {
                provider: self.provider,
                api: ApiType::OpenAiResponses.as_str().to_string(),
                model: self.model,
                items: self.replay.into_iter().map(|(_, v)| v).collect(),
            });
        }
        self.reply
    }
}

fn has_encrypted_content(item: &Value) -> bool {
    item.get("encrypted_content")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
}

fn usage(u: &Value) -> Usage {
    let n = |v: Option<&Value>| v.and_then(Value::as_u64).unwrap_or(0);
    Usage {
        input: n(u.get("input_tokens")),
        output: n(u.get("output_tokens")),
        cached: n(u
            .get("input_tokens_details")
            .and_then(|d| d.get("cached_tokens"))),
        cache_write: 0,
    }
}

fn map_stream_error(e: &Value) -> Error {
    let code = match e.get("code") {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(500) as u16,
        Some(Value::String(s)) => match s.as_str() {
            "rate_limit_exceeded" => 429,
            "server_error" => 500,
            "service_unavailable" => 503,
            "invalid_api_key" | "authentication_error" => 401,
            other => other.parse::<u16>().unwrap_or(400),
        },
        _ => match e.get("type").and_then(Value::as_str) {
            Some("rate_limit_error") => 429,
            Some("invalid_request_error") => 400,
            Some("authentication_error") => 401,
            _ => 500,
        },
    };
    Error::Status(code, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_done_preserves_call_id_and_reconciles_replay_arguments() {
        let mut acc = Accumulator::new("resp", "gpt-5");
        let mut deltas = String::new();
        let events = [
            json!({"type":"response.output_item.added","output_index":0,"item":{"id":"fc_1","type":"function_call","call_id":"call_real","name":"lua","arguments":""}}),
            json!({"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":0,"delta":"{\"code\":\"draft\"}"}),
            json!({"type":"response.function_call_arguments.done","item_id":"fc_1","output_index":0,"arguments":"{\"code\":\"final()\"}"}),
            json!({"type":"response.output_item.done","output_index":0,"item":{"id":"fc_1","type":"function_call"}}),
            json!({"type":"response.completed","response":{"status":"completed"}}),
        ];
        for e in &events {
            acc.event("", e, &mut |d| {
                if let Delta::Tool { code, .. } = d {
                    deltas.push_str(code);
                }
            })
            .unwrap();
        }
        let r = acc.finish();
        assert_eq!(deltas, "draft");
        assert_eq!(r.tool_calls[0].id, "call_real");
        assert_eq!(r.tool_calls[0].name, "lua");
        assert_eq!(r.tool_calls[0].arguments, "{\"code\":\"final()\"}");
        let item = &r.replay.unwrap().items[0];
        assert_eq!(item["call_id"], "call_real");
        assert_eq!(item["name"], "lua");
        assert_eq!(item["arguments"], "{\"code\":\"final()\"}");
    }
}
