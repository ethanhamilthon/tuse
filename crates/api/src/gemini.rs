use crate::args::ArgStream;
use crate::client::{
    Client, finalize_calls, map_err, parse_data_url, parse_json, parse_json_object, read_body,
    read_sse, url_encode,
};
use crate::types::{ApiType, Delta, Error, Message, Replay, Reply, Request, ToolCall, Usage};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

const MAX_MODEL_PAGES: usize = 10;

pub(crate) fn encode(req: &Request) -> Result<Value, Error> {
    let mut contents: Vec<Value> = vec![];
    for m in &req.messages {
        let (role, parts) = match m {
            Message::User { text, images } => {
                let mut parts = vec![];
                if !text.is_empty() {
                    parts.push(json!({"text": text}));
                }
                for url in images {
                    let (mime, data) = parse_data_url(url, &req.provider, ApiType::GoogleGemini)?;
                    parts.push(inline_data(mime, data));
                }
                ("user", parts)
            }
            Message::Assistant {
                text,
                tool_calls,
                replay,
            } => {
                let mut parts = vec![];
                if let Some(r) = replay
                    .as_ref()
                    .filter(|r| r.matches(ApiType::GoogleGemini, &req.model))
                {
                    parts.extend(r.items.iter().cloned());
                } else {
                    if !text.is_empty() {
                        parts.push(json!({"text": text}));
                    }
                    for c in tool_calls {
                        parts.push(json!({
                            "functionCall": {
                                "id": c.id,
                                "name": c.name,
                                "args": parse_json_object(&c.arguments),
                            }
                        }));
                    }
                }
                ("model", parts)
            }
            Message::ToolResult {
                id,
                name,
                output,
                images,
            } => {
                let mut parts = vec![json!({
                    "functionResponse": {
                        "id": id,
                        "name": name,
                        "response": {"output": output},
                    }
                })];
                if let Some(im) = images {
                    for url in &im.urls {
                        let (mime, data) =
                            parse_data_url(url, &req.provider, ApiType::GoogleGemini)?;
                        parts.push(inline_data(mime, data));
                    }
                }
                ("user", parts)
            }
        };
        if parts.is_empty() {
            continue;
        }
        if let Some(last) = contents.last_mut()
            && last["role"] == role
            && let Some(arr) = last["parts"].as_array_mut()
        {
            arr.extend(parts);
        } else {
            contents.push(json!({"role": role, "parts": parts}));
        }
    }
    let mut body = json!({"contents": contents});
    if !req.system.is_empty() {
        body["systemInstruction"] = json!({"parts": [{"text": req.system}]});
    }
    if !req.tools.is_empty() {
        body["tools"] = json!([{
            "functionDeclarations": req.tools.iter().map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            }).collect::<Vec<_>>()
        }]);
    }
    if let Some(effort) = req.effort.as_deref() {
        let thinking = if req.model.starts_with("gemini-2.5-") {
            let budget = match effort {
                "low" => 1024,
                "medium" => 8192,
                _ => 24576,
            };
            json!({"includeThoughts": true, "thinkingBudget": budget})
        } else {
            json!({"includeThoughts": true, "thinkingLevel": effort})
        };
        body["generationConfig"] = json!({"thinkingConfig": thinking});
    }
    Ok(body)
}

fn inline_data(mime: &str, data: &str) -> Value {
    json!({"inlineData": {"mimeType": mime, "data": data}})
}

pub(crate) fn complete(
    client: &Client,
    req: &Request,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<Reply, Error> {
    let body = encode(req)?;
    let url = format!(
        "{}/models/{}:streamGenerateContent?alt=sse",
        client.base_url, req.model
    );
    let resp = client
        .agent
        .post(&url)
        .set("x-goog-api-key", &client.api_key)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(map_err)?;
    let streaming = resp.content_type().contains("event-stream");
    let mut acc = Accumulator::new(&req.model);
    if !streaming {
        let v = parse_json(&read_body(resp)?)?;
        match v {
            Value::Array(chunks) => {
                for c in &chunks {
                    acc.chunk(c, on_delta)?;
                }
            }
            Value::Object(_) => acc.chunk(&v, on_delta)?,
            _ => return Err(Error::Parse("expected JSON object or array".into())),
        }
        return Ok(acc.finish());
    }
    read_sse(resp.into_reader(), cancel, |_, data| {
        let v = parse_json(data)?;
        acc.chunk(&v, on_delta)?;
        Ok(true)
    })?;
    Ok(acc.finish())
}

pub(crate) fn models(client: &Client) -> Result<Vec<String>, Error> {
    let mut out = vec![];
    let mut seen = HashSet::new();
    let mut token: Option<String> = None;
    for _ in 0..MAX_MODEL_PAGES {
        let url = match &token {
            Some(t) => format!(
                "{}/models?pageSize=100&pageToken={}",
                client.base_url,
                url_encode(t)
            ),
            None => format!("{}/models?pageSize=100", client.base_url),
        };
        let resp = client
            .agent
            .get(&url)
            .set("x-goog-api-key", &client.api_key)
            .call()
            .map_err(map_err)?;
        let v = parse_json(&read_body(resp)?)?;
        for m in v["models"].as_array().into_iter().flatten() {
            if let Some(methods) = m
                .get("supportedGenerationMethods")
                .and_then(Value::as_array)
                && !methods
                    .iter()
                    .any(|x| x.as_str() == Some("generateContent"))
            {
                continue;
            }
            if let Some(name) = m.get("name").and_then(Value::as_str) {
                out.push(name.strip_prefix("models/").unwrap_or(name).to_string());
            }
        }
        let next = v
            .get("nextPageToken")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        match next {
            Some(t) if seen.insert(t.to_string()) => token = Some(t.to_string()),
            _ => break,
        }
    }
    Ok(out)
}

struct Accumulator {
    model: String,
    reply: Reply,
    replay: Vec<Value>,
}

impl Accumulator {
    fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            reply: Reply::default(),
            replay: vec![],
        }
    }

    fn chunk(&mut self, v: &Value, on_delta: &mut dyn FnMut(Delta)) -> Result<(), Error> {
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            return Err(Error::Status(
                e.get("code").and_then(Value::as_u64).unwrap_or(500) as u16,
                e.to_string(),
            ));
        }
        if let Some(u) = v.get("usageMetadata").filter(|u| u.is_object()) {
            self.reply.usage = Some(usage(u));
        }
        let Some(cand) = v.get("candidates").and_then(|c| c.get(0)) else {
            return Ok(());
        };
        if let Some(f) = cand.get("finishReason").and_then(Value::as_str) {
            self.reply.finish = f.to_string();
        }
        for part in cand
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            push_replay_part(&mut self.replay, part);
            if let Some(t) = part.get("text").and_then(Value::as_str)
                && !t.is_empty()
            {
                if part.get("thought").and_then(Value::as_bool) == Some(true) {
                    self.reply.reasoning.push_str(t);
                    on_delta(Delta::Reasoning(t));
                } else {
                    self.reply.text.push_str(t);
                    on_delta(Delta::Text(t));
                }
            }
            if let Some(fc) = part.get("functionCall") {
                let name = fc.get("name").and_then(Value::as_str).unwrap_or("");
                let idx = self.reply.tool_calls.len();
                let id = fc
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .unwrap_or_else(|| format!("call_{idx}"));
                let args = fc.get("args").cloned().unwrap_or_else(|| json!({}));
                let arguments = serde_json::to_string(&args).unwrap_or_else(|_| "{}".into());
                let code = ArgStream::default().feed(&arguments);
                self.reply.tool_calls.push(ToolCall {
                    id,
                    name: name.to_string(),
                    arguments,
                });
                let call = &self.reply.tool_calls[idx];
                if !call.name.is_empty() {
                    on_delta(Delta::Tool {
                        index: idx,
                        id: &call.id,
                        name: &call.name,
                        code: &code,
                    });
                }
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Reply {
        finalize_calls(&mut self.reply.tool_calls);
        if !self.replay.is_empty() {
            self.reply.replay = Some(Replay {
                api: ApiType::GoogleGemini.as_str().to_string(),
                model: self.model,
                items: self.replay,
            });
        }
        self.reply
    }
}

fn plain_text_part(v: &Value) -> Option<(bool, &str)> {
    let obj = v.as_object()?;
    if !obj.keys().all(|k| k == "text" || k == "thought") {
        return None;
    }
    let text = obj.get("text")?.as_str()?;
    let thought = obj.get("thought").and_then(Value::as_bool).unwrap_or(false);
    Some((thought, text))
}

fn push_replay_part(replay: &mut Vec<Value>, part: &Value) {
    if let Some((thought, text)) = plain_text_part(part)
        && let Some(prev) = replay.last_mut()
        && let Some((prev_thought, _)) = plain_text_part(prev)
        && prev_thought == thought
        && let Some(prev_text) = prev.get("text").and_then(Value::as_str).map(String::from)
    {
        prev["text"] = Value::String(format!("{prev_text}{text}"));
        return;
    }
    replay.push(part.clone());
}

fn usage(u: &Value) -> Usage {
    let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
    Usage {
        input: n("promptTokenCount"),
        output: n("candidatesTokenCount") + n("thoughtsTokenCount"),
        cached: n("cachedContentTokenCount"),
        cache_write: 0,
    }
}
