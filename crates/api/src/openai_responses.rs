use crate::args::ArgStream;
use crate::http::{
    Auth, Conn, add_reasoning, add_text, check_err, drive_response, finalize_calls,
    flush_tool_images, jarr, jerr, jstr, ju64, make_replay, map_json_error,
};
use crate::types::{ApiType, Delta, Error, Message, Reply, Request, ToolCall, ToolImage, Usage};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn img_part(url: &str) -> Value {
    json!({"type": "input_image", "image_url": url})
}

pub(crate) fn encode(req: &Request) -> Value {
    let (mut input, mut images): (Vec<Value>, Vec<&ToolImage>) = (vec![], vec![]);
    for m in &req.messages {
        match m {
            Message::User { text, images: im } => {
                flush_tool_images(&mut input, &mut images, "input_text", img_part);
                let mut parts = vec![json!({"type": "input_text", "text": text})];
                parts.extend(im.iter().map(|u| img_part(u)));
                input.push(json!({"role": "user", "content": parts}));
            }
            Message::Assistant {
                text,
                tool_calls,
                replay,
            } => {
                flush_tool_images(&mut input, &mut images, "input_text", img_part);
                let hit = replay
                    .as_ref()
                    .filter(|r| r.matches(&req.provider, ApiType::OpenAiResponses, &req.model));
                if let Some(r) = hit {
                    input.extend(r.items.iter().cloned());
                } else {
                    if !text.is_empty() {
                        input.push(json!({"role": "assistant", "content": [{"type": "output_text", "text": text}]}));
                    }
                    for c in tool_calls {
                        input.push(json!({"type": "function_call", "call_id": c.id, "name": c.name, "arguments": c.arguments}));
                    }
                }
            }
            Message::ToolResult {
                id,
                output,
                images: im,
                ..
            } => {
                input
                    .push(json!({"type": "function_call_output", "call_id": id, "output": output}));
                if let Some(im) = im.as_ref().filter(|im| !im.urls.is_empty()) {
                    images.push(im);
                }
            }
        }
    }
    flush_tool_images(&mut input, &mut images, "input_text", img_part);
    let mut body = json!({"model": req.model, "input": input, "stream": true, "store": false});
    if !req.system.is_empty() {
        body["instructions"] = json!(req.system);
    }
    if !req.tools.is_empty() {
        let ts: Vec<_> = req.tools.iter().map(|t| json!({"type": "function", "name": t.name, "description": t.description, "parameters": t.parameters})).collect();
        body["tools"] = Value::Array(ts);
    }
    if let Some(e) = &req.effort {
        body["reasoning"] = json!({"effort": e, "summary": "auto"});
        body["include"] = json!(["reasoning.encrypted_content"]);
    }
    body
}

pub(crate) fn complete(
    conn: &Conn,
    req: &Request,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<Reply, Error> {
    let resp = conn.post_json("/responses", &Auth::Bearer, &[], &encode(req))?;
    let mut acc = Acc::default();
    let (streamed, _) = drive_response(resp, cancel, |ev, v| {
        match ev {
            None => acc.fallback(&v, on_delta)?,
            Some(ev) => acc.event(ev, &v, on_delta)?,
        }
        Ok(!acc.completed)
    })?;
    if streamed && !acc.completed {
        return Err(Error::Transport(
            "stream ended before response.completed or response.incomplete".into(),
        ));
    }
    acc.reply.tool_calls = acc.slots.into_iter().map(|s| s.call).collect();
    finalize_calls(&mut acc.reply.tool_calls);
    acc.replay.sort_by_key(|(i, _)| *i);
    let items = acc.replay.into_iter().map(|(_, v)| v).collect();
    acc.reply.replay = make_replay(
        req.provider.clone(),
        ApiType::OpenAiResponses,
        req.model.clone(),
        items,
    );
    Ok(acc.reply)
}

#[derive(Default)]
struct Slot {
    oi: Option<usize>,
    item_id: String,
    call: ToolCall,
    args: ArgStream,
    streamed: bool,
    announced: bool,
}

#[derive(Default)]
struct Acc {
    reply: Reply,
    slots: Vec<Slot>,
    replay: Vec<(usize, Value)>,
    completed: bool,
}

impl Acc {
    fn slot(&mut self, item_id: &str, oi: Option<usize>) -> usize {
        if !item_id.is_empty()
            && let Some(i) = self.slots.iter().position(|s| s.item_id == item_id)
        {
            return i;
        }
        if let Some(oi) = oi
            && let Some(i) = self.slots.iter().position(|s| s.oi == Some(oi))
        {
            return i;
        }
        self.slots.push(Slot {
            oi,
            item_id: item_id.into(),
            ..Default::default()
        });
        self.slots.len() - 1
    }

    fn emit(&mut self, idx: usize, code: &str, on_delta: &mut dyn FnMut(Delta)) {
        let s = &mut self.slots[idx];
        if !s.call.name.is_empty() && (!s.announced || !code.is_empty()) {
            s.announced = true;
            on_delta(Delta::Tool {
                index: idx,
                id: &s.call.id,
                name: &s.call.name,
                code,
            });
        }
    }

    fn sync_call(
        &mut self,
        item_id: &str,
        oi: Option<usize>,
        v: &Value,
        append: bool,
        on_delta: &mut dyn FnMut(Delta),
    ) -> usize {
        let idx = self.slot(item_id, oi);
        let s = &mut self.slots[idx];
        if !item_id.is_empty() {
            s.item_id = item_id.into();
        }
        if oi.is_some() {
            s.oi = oi;
        }
        if let Some(cid) = jstr(v, "call_id").filter(|c| !c.is_empty()) {
            s.call.id = cid.into();
        } else if s.call.id.is_empty() && !item_id.is_empty() {
            s.call.id = item_id.into();
        }
        if let Some(n) = jstr(v, "name").filter(|n| !n.is_empty())
            && s.call.name.is_empty()
        {
            s.call.name = n.into();
        }
        let mut code = None;
        if let Some(a) = jstr(v, "arguments").filter(|a| !a.is_empty()) {
            if append {
                s.call.arguments.push_str(a);
                s.streamed = true;
                code = Some(s.args.feed(a));
            } else {
                s.call.arguments = a.into();
                if !s.streamed {
                    s.streamed = true;
                    code = Some(s.args.feed(a));
                }
            }
        }
        if append || code.is_some() {
            self.emit(idx, code.as_deref().unwrap_or(""), on_delta);
        }
        idx
    }

    fn event(&mut self, ev: &str, v: &Value, on_delta: &mut dyn FnMut(Delta)) -> Result<(), Error> {
        check_err(v)?;
        let oi = ju64(v, "output_index").map(|n| n as usize);
        match jstr(v, "type").unwrap_or(ev) {
            "response.output_text.delta" => {
                if let Some(t) = jstr(v, "delta") {
                    add_text(&mut self.reply, t, on_delta);
                }
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                if let Some(t) = jstr(v, "delta") {
                    add_reasoning(&mut self.reply, t, on_delta);
                }
            }
            "response.output_item.added" => {
                if let Some(it) = v
                    .get("item")
                    .filter(|i| jstr(i, "type") == Some("function_call"))
                {
                    self.sync_call(jstr(it, "id").unwrap_or(""), oi, it, true, on_delta);
                }
            }
            "response.function_call_arguments.delta" => {
                let idx = self.slot(jstr(v, "item_id").unwrap_or(""), oi);
                let (d, s) = (jstr(v, "delta").unwrap_or(""), &mut self.slots[idx]);
                if !d.is_empty() {
                    s.call.arguments.push_str(d);
                    s.streamed = true;
                }
                let code = s.args.feed(d);
                self.emit(idx, &code, on_delta);
            }
            "response.function_call_arguments.done" => {
                self.sync_call(jstr(v, "item_id").unwrap_or(""), oi, v, false, on_delta);
            }
            "response.output_item.done" => {
                if let Some(it) = v.get("item") {
                    self.item_done(oi, it, on_delta);
                }
            }
            "response.completed" | "response.incomplete" => {
                self.completed = true;
                if let Some(r) = v.get("response") {
                    self.meta(r);
                }
            }
            "response.failed" => {
                return Err(map_json_error(
                    v.get("response").and_then(jerr).unwrap_or(v),
                ));
            }
            _ => {}
        }
        Ok(())
    }

    fn item_done(&mut self, oi: Option<usize>, item: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let order = oi.unwrap_or(self.replay.len());
        match jstr(item, "type") {
            Some("function_call") => {
                let idx = self.sync_call(jstr(item, "id").unwrap_or(""), oi, item, false, on_delta);
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
            Some("reasoning") if jstr(item, "encrypted_content").is_some_and(|s| !s.is_empty()) => {
                self.replay.push((order, item.clone()));
            }
            Some("message") => self.replay.push((order, item.clone())),
            _ => {}
        }
    }

    fn meta(&mut self, r: &Value) {
        if let Some(st) = jstr(r, "status") {
            self.reply.finish = st.into();
        }
        if let Some(u) = r.get("usage").filter(|u| u.is_object()) {
            let cache_read = u
                .get("input_tokens_details")
                .and_then(|d| ju64(d, "cached_tokens"))
                .unwrap_or(0);
            self.reply.usage = Some(Usage {
                input: ju64(u, "input_tokens").unwrap_or(0),
                output: ju64(u, "output_tokens").unwrap_or(0),
                cache_read,
                cache_write: 0,
            });
        }
    }

    fn fallback(&mut self, v: &Value, on_delta: &mut dyn FnMut(Delta)) -> Result<(), Error> {
        check_err(v)?;
        if jstr(v, "status") == Some("failed") {
            return Err(map_json_error(v));
        }
        self.meta(v);
        for (i, it) in jarr(v, "output").iter().enumerate() {
            match jstr(it, "type") {
                Some("reasoning") => {
                    for s in jarr(it, "summary") {
                        if let Some(t) = jstr(s, "text") {
                            add_reasoning(&mut self.reply, t, on_delta);
                        }
                    }
                    self.item_done(Some(i), it, on_delta);
                }
                Some("message") => {
                    for c in jarr(it, "content") {
                        if jstr(c, "type") == Some("output_text")
                            && let Some(t) = jstr(c, "text")
                        {
                            add_text(&mut self.reply, t, on_delta);
                        }
                    }
                    self.replay.push((i, it.clone()));
                }
                Some("function_call") => self.item_done(Some(i), it, on_delta),
                _ => {}
            }
        }
        Ok(())
    }
}
