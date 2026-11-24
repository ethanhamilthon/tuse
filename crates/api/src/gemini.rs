use crate::client::Client;
use crate::http::{
    active_turn_start, add_reasoning, add_text, check_err, drive_response, finalize_calls, jarr,
    jstr, ju64, make_replay, paginate_models, parse_data_url, parse_json_object, post_json,
    push_tool, push_turn, url_encode,
};
use crate::types::{ApiType, Delta, Error, Message, Reply, Request, Usage};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

const SKIP_SIG: &str = "skip_thought_signature_validator";

fn push_images(parts: &mut Vec<Value>, urls: &[String], prov: &str) -> Result<(), Error> {
    for u in urls {
        let (mime, data) = parse_data_url(u, prov, ApiType::GoogleGemini)?;
        parts.push(json!({"inlineData": {"mimeType": mime, "data": data}}));
    }
    Ok(())
}

pub(crate) fn encode(req: &Request) -> Result<Value, Error> {
    let active_start = active_turn_start(&req.messages);
    let (mut contents, mut wire_ids) = (vec![], HashMap::<&str, bool>::new());
    for (idx, m) in req.messages.iter().enumerate() {
        let (role, parts) = match m {
            Message::User { text, images } => {
                let mut parts = vec![];
                if !text.is_empty() {
                    parts.push(json!({"text": text}));
                }
                push_images(&mut parts, images, &req.provider)?;
                ("user", parts)
            }
            Message::Assistant {
                text,
                tool_calls,
                replay,
            } => {
                wire_ids.clear();
                let mut parts = vec![];
                let hit = replay
                    .as_ref()
                    .filter(|r| r.matches(&req.provider, ApiType::GoogleGemini, &req.model));
                if let Some(r) = hit {
                    let mut items = r.items.clone();
                    let has_sig = items.iter().any(|p| p.get("thoughtSignature").is_some());
                    let mut fc_i = 0usize;
                    for p in &mut items {
                        if let Some(fc) = p.get("functionCall") {
                            let has_id = jstr(fc, "id").is_some_and(|s| !s.is_empty());
                            if let Some(tc) = tool_calls.get(fc_i) {
                                wire_ids.insert(tc.id.as_str(), has_id);
                            }
                            let need_sig = idx >= active_start
                                && !has_sig
                                && fc_i == 0
                                && !req.model.starts_with("gemini-2.5-");
                            if need_sig && let Some(obj) = p.as_object_mut() {
                                obj.insert("thoughtSignature".into(), json!(SKIP_SIG));
                            }
                            fc_i += 1;
                        }
                    }
                    parts.extend(items);
                } else {
                    if !text.is_empty() {
                        parts.push(json!({"text": text}));
                    }
                    for c in tool_calls {
                        let mut fc =
                            json!({"name": c.name, "args": parse_json_object(&c.arguments)});
                        if !c.id.is_empty() {
                            fc["id"] = json!(c.id);
                            wire_ids.insert(c.id.as_str(), true);
                        }
                        let mut part = json!({"functionCall": fc});
                        if idx >= active_start {
                            part["thoughtSignature"] = json!(SKIP_SIG);
                        }
                        parts.push(part);
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
                let mut fr = json!({"name": name, "response": {"output": output}});
                if wire_ids.get(id.as_str()).copied().unwrap_or(!id.is_empty()) && !id.is_empty() {
                    fr["id"] = json!(id);
                }
                let mut parts = vec![json!({"functionResponse": fr})];
                if let Some(im) = images {
                    push_images(&mut parts, &im.urls, &req.provider)?;
                }
                ("user", parts)
            }
        };
        push_turn(&mut contents, role, "parts", parts);
    }
    let mut body = json!({"contents": contents});
    if !req.system.is_empty() {
        body["systemInstruction"] = json!({"parts": [{"text": req.system}]});
    }
    if !req.tools.is_empty() {
        let decls: Vec<_> = req.tools.iter().map(|t| json!({"name": t.name, "description": t.description, "parameters": t.parameters})).collect();
        body["tools"] = json!([{"functionDeclarations": decls}]);
    }
    if let Some(effort) = req.effort.as_deref() {
        let thinking = if req.model.starts_with("gemini-2.5-") {
            let b = match effort {
                "low" => 1024,
                "medium" => 8192,
                _ => 24576,
            };
            json!({"includeThoughts": true, "thinkingBudget": b})
        } else {
            json!({"includeThoughts": true, "thinkingLevel": effort})
        };
        body["generationConfig"] = json!({"thinkingConfig": thinking});
    }
    Ok(body)
}

pub(crate) fn complete(
    client: &Client,
    req: &Request,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<Reply, Error> {
    let url = format!(
        "{}/models/{}:streamGenerateContent?alt=sse",
        client.base_url, req.model
    );
    let resp = post_json(
        client,
        &url,
        &[("x-goog-api-key", &client.api_key)],
        &encode(req)?,
    )?;
    let (mut reply, mut replay) = (Reply::default(), vec![]);
    let (streamed, _) = drive_response(resp, cancel, |ev, v| {
        match (ev, &v) {
            (None, Value::Array(cs)) => cs
                .iter()
                .try_for_each(|c| chunk(&mut reply, &mut replay, c, on_delta))?,
            (None, Value::Object(_)) | (Some(_), _) => {
                chunk(&mut reply, &mut replay, &v, on_delta)?
            }
            _ => return Err(Error::Parse("expected JSON object or array".into())),
        }
        Ok(true)
    })?;
    if streamed && reply.finish.is_empty() {
        return Err(Error::Transport("stream ended before finishReason".into()));
    }
    finalize_calls(&mut reply.tool_calls);
    reply.replay = make_replay(
        req.provider.clone(),
        ApiType::GoogleGemini,
        req.model.clone(),
        replay,
    );
    Ok(reply)
}

pub(crate) fn models(client: &Client) -> Result<Vec<String>, Error> {
    let base = &client.base_url;
    paginate_models(
        client,
        &[("x-goog-api-key", client.api_key.as_str())],
        |cur| match cur {
            Some(t) => format!("{base}/models?pageSize=100&pageToken={}", url_encode(t)),
            None => format!("{base}/models?pageSize=100"),
        },
        |v, out| {
            for m in jarr(v, "models") {
                let ok = m
                    .get("supportedGenerationMethods")
                    .and_then(Value::as_array)
                    .is_none_or(|ms| ms.iter().any(|x| x.as_str() == Some("generateContent")));
                if ok && let Some(name) = jstr(m, "name") {
                    out.push(name.strip_prefix("models/").unwrap_or(name).into());
                }
            }
            jstr(v, "nextPageToken").map(String::from)
        },
    )
}

fn chunk(
    reply: &mut Reply,
    replay: &mut Vec<Value>,
    v: &Value,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<(), Error> {
    check_err(v)?;
    if let Some(u) = v.get("usageMetadata").filter(|u| u.is_object()) {
        let out = ju64(u, "candidatesTokenCount").unwrap_or(0)
            + ju64(u, "thoughtsTokenCount").unwrap_or(0);
        reply.usage = Some(Usage {
            input: ju64(u, "promptTokenCount").unwrap_or(0),
            output: out,
            cached: ju64(u, "cachedContentTokenCount").unwrap_or(0),
            cache_write: 0,
        });
    }
    let Some(cand) = jarr(v, "candidates").first() else {
        return Ok(());
    };
    if let Some(f) = jstr(cand, "finishReason") {
        reply.finish = f.into();
    }
    for part in cand.get("content").map_or(&[][..], |c| jarr(c, "parts")) {
        push_replay_part(replay, part);
        if let Some(t) = jstr(part, "text") {
            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                add_reasoning(reply, t, on_delta);
            } else {
                add_text(reply, t, on_delta);
            }
        }
        if let Some(fc) = part.get("functionCall") {
            let idx = reply.tool_calls.len();
            let id = jstr(fc, "id")
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| format!("call_{idx}"));
            let args = serde_json::to_string(fc.get("args").unwrap_or(&json!({})))
                .unwrap_or_else(|_| "{}".into());
            push_tool(
                &mut reply.tool_calls,
                None,
                id,
                jstr(fc, "name").unwrap_or("").into(),
                args,
                on_delta,
            );
        }
    }
    Ok(())
}

fn plain_text_part(v: &Value) -> Option<(bool, &str)> {
    let obj = v.as_object()?;
    if !obj.keys().all(|k| k == "text" || k == "thought") {
        return None;
    }
    Some((
        obj.get("thought").and_then(Value::as_bool).unwrap_or(false),
        obj.get("text")?.as_str()?,
    ))
}

fn push_replay_part(replay: &mut Vec<Value>, part: &Value) {
    if let Some((th, text)) = plain_text_part(part)
        && let Some(prev) = replay.last_mut()
        && plain_text_part(prev).is_some_and(|(pt, _)| pt == th)
        && let Some(old) = jstr(prev, "text").map(String::from)
    {
        prev["text"] = Value::String(format!("{old}{text}"));
        return;
    }
    replay.push(part.clone());
}
