use crate::args::ArgStream;
use crate::client::Client;
use crate::types::{ApiType, Delta, Error, Message, Replay, Reply, ToolCall, ToolImage, one_line};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn jstr<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

pub(crate) fn ju64(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(Value::as_u64)
}

pub(crate) fn jarr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

pub(crate) fn jerr(v: &Value) -> Option<&Value> {
    v.get("error").filter(|e| !e.is_null())
}

pub(crate) fn add_text(reply: &mut Reply, t: &str, on_delta: &mut dyn FnMut(Delta)) {
    if !t.is_empty() {
        reply.text.push_str(t);
        on_delta(Delta::Text(t));
    }
}

pub(crate) fn add_reasoning(reply: &mut Reply, t: &str, on_delta: &mut dyn FnMut(Delta)) {
    if !t.is_empty() {
        reply.reasoning.push_str(t);
        on_delta(Delta::Reasoning(t));
    }
}

pub(crate) fn push_tool(
    calls: &mut Vec<ToolCall>,
    args: Option<&mut Vec<ArgStream>>,
    id: String,
    name: String,
    arguments: String,
    on_delta: &mut dyn FnMut(Delta),
) -> usize {
    let slot = calls.len();
    let mut stream = ArgStream::default();
    let code = if arguments.is_empty() {
        String::new()
    } else {
        stream.feed(&arguments)
    };
    if let Some(a) = args {
        a.push(stream);
    }
    calls.push(ToolCall {
        id,
        name,
        arguments,
    });
    let c = &calls[slot];
    if !c.name.is_empty() {
        on_delta(Delta::Tool {
            index: slot,
            id: &c.id,
            name: &c.name,
            code: &code,
        });
    }
    slot
}

pub(crate) fn check_err(v: &Value) -> Result<(), Error> {
    if jstr(v, "type") == Some("error") {
        return Err(map_json_error(v.get("error").unwrap_or(v)));
    }
    if let Some(e) = jerr(v) {
        return Err(map_json_error(e));
    }
    Ok(())
}

pub(crate) fn map_err(e: ureq::Error) -> Error {
    match e {
        ureq::Error::Status(c, r) => Error::Status(c, r.into_string().unwrap_or_default()),
        ureq::Error::Transport(t) => Error::Transport(t.to_string()),
    }
}

pub(crate) fn check_status(resp: ureq::Response) -> Result<ureq::Response, Error> {
    let c = resp.status();
    if (300..400).contains(&c) {
        let loc = resp.header("location").unwrap_or("(unknown)");
        return Err(Error::Status(c, format!("unexpected redirect to {loc}")));
    }
    Ok(resp)
}

pub(crate) fn post_json(
    c: &Client,
    url: &str,
    hdrs: &[(&str, &str)],
    body: &Value,
) -> Result<ureq::Response, Error> {
    let req = hdrs.iter().fold(
        c.agent.post(url).set("Content-Type", "application/json"),
        |r, (k, v)| r.set(k, v),
    );
    check_status(req.send_string(&body.to_string()).map_err(map_err)?)
}

pub(crate) fn get_json(c: &Client, url: &str, hdrs: &[(&str, &str)]) -> Result<Value, Error> {
    let req = hdrs.iter().fold(c.agent.get(url), |r, (k, v)| r.set(k, v));
    let mut text = String::new();
    check_status(req.call().map_err(map_err)?)?
        .into_reader()
        .read_to_string(&mut text)
        .map_err(|e| Error::Transport(e.to_string()))?;
    parse_json(&text)
}

pub(crate) fn drive_response(
    resp: ureq::Response,
    cancel: &AtomicBool,
    mut on_frame: impl FnMut(Option<&str>, Value) -> Result<bool, Error>,
) -> Result<(bool, bool), Error> {
    if !resp.content_type().contains("event-stream") {
        let mut text = String::new();
        resp.into_reader()
            .read_to_string(&mut text)
            .map_err(|e| Error::Transport(e.to_string()))?;
        on_frame(None, parse_json(&text)?)?;
        return Ok((false, true));
    }
    let mut ev = String::new();
    for line in BufReader::new(resp.into_reader()).lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let line = line.map_err(|e| Error::Transport(e.to_string()))?;
        if line.is_empty() {
            ev.clear();
        } else if let Some(e) = line.strip_prefix("event:") {
            ev = e.trim().into();
        } else if let Some(d) = line.strip_prefix("data:").map(str::trim) {
            if d == "[DONE]" {
                return Ok((true, true));
            }
            if !d.is_empty() && !on_frame(Some(&ev), parse_json(d)?)? {
                return Ok((true, true));
            }
        }
    }
    Ok((true, false))
}

pub(crate) fn parse_json(text: &str) -> Result<Value, Error> {
    serde_json::from_str(text).map_err(|e| Error::Parse(e.to_string()))
}

pub(crate) fn parse_json_object(raw: &str) -> Value {
    serde_json::from_str::<Value>(raw)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

pub(crate) fn map_json_error(e: &Value) -> Error {
    let code = match e.get("code") {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(500) as u16,
        Some(Value::String(s)) => match s.as_str() {
            "rate_limit_exceeded" => 429,
            "server_error" => 500,
            "service_unavailable" => 503,
            "invalid_api_key" | "authentication_error" => 401,
            other => other.parse::<u16>().unwrap_or(400),
        },
        _ => match jstr(e, "type") {
            Some("rate_limit_error") => 429,
            Some("overloaded_error") => 529,
            Some("invalid_request_error") => 400,
            Some("authentication_error") => 401,
            Some("permission_error") => 403,
            Some("not_found_error") => 404,
            Some("request_too_large") => 413,
            _ => 500,
        },
    };
    Error::Status(code, e.to_string())
}

pub(crate) fn parse_data_url<'a>(
    url: &'a str,
    prov: &str,
    api: ApiType,
) -> Result<(&'a str, &'a str), Error> {
    let fail = || {
        let (a, u) = (api.as_str(), one_line(url));
        Error::Unsupported(format!(
            "provider `{prov}` ({a}) does not support image `{u}` (requires base64 data:<mime>;base64,<data> URL)"
        ))
    };
    let (mime, data) = url
        .strip_prefix("data:")
        .and_then(|r| r.split_once(";base64,"))
        .ok_or_else(fail)?;
    let ok_mime = !mime.is_empty() && mime.contains('/') && !mime.chars().any(char::is_whitespace);
    if !ok_mime || !valid_b64(data) {
        return Err(fail());
    }
    Ok((mime, data))
}

fn valid_b64(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || !b.len().is_multiple_of(4) {
        return false;
    }
    let pad = b.iter().rev().take_while(|&&x| x == b'=').count();
    let body = &b[..b.len() - pad];
    let val = |x: u8| match x {
        b'A'..=b'Z' => Some(x - b'A'),
        b'a'..=b'z' => Some(x - b'a' + 26),
        b'0'..=b'9' => Some(x - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    if pad > 2 || body.is_empty() || body.iter().any(|&x| val(x).is_none()) {
        return false;
    }
    match pad {
        1 => val(body[body.len() - 1]).is_some_and(|v| v & 0x03 == 0),
        2 => val(body[body.len() - 1]).is_some_and(|v| v & 0x0f == 0),
        _ => true,
    }
}

pub(crate) fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub(crate) fn paginate_models(
    c: &Client,
    hdrs: &[(&str, &str)],
    make_url: impl Fn(Option<&str>) -> String,
    extract: impl Fn(&Value, &mut Vec<String>) -> Option<String>,
) -> Result<Vec<String>, Error> {
    let (mut out, mut seen, mut cur) = (vec![], HashSet::new(), None);
    for _ in 0..10 {
        let v = get_json(c, &make_url(cur.as_deref()), hdrs)?;
        match extract(&v, &mut out) {
            Some(next) if !next.is_empty() && seen.insert(next.clone()) => cur = Some(next),
            _ => break,
        }
    }
    Ok(out)
}

pub(crate) fn openai_models(c: &Client) -> Result<Vec<String>, Error> {
    let auth = format!("Bearer {}", c.api_key);
    let v = get_json(
        c,
        &format!("{}/models", c.base_url),
        &[("Authorization", &auth)],
    )?;
    let ids = jarr(&v, "data")
        .iter()
        .filter_map(|m| jstr(m, "id").map(String::from));
    Ok(ids.collect())
}

pub(crate) fn push_turn(out: &mut Vec<Value>, role: &str, field: &str, items: Vec<Value>) {
    if items.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut()
        && last["role"] == role
        && let Some(arr) = last[field].as_array_mut()
    {
        arr.extend(items);
    } else {
        out.push(json!({"role": role, field: items}));
    }
}

pub(crate) fn flush_tool_images(
    out: &mut Vec<Value>,
    images: &mut Vec<&ToolImage>,
    ty: &str,
    make_img: impl Fn(&str) -> Value,
) {
    if images.is_empty() {
        return;
    }
    let mut parts = vec![json!({"type": ty, "text": "[Images from the tool results above]"})];
    for im in images.drain(..) {
        parts.push(json!({"type": ty, "text": format!("[Image: {}]", im.label)}));
        parts.extend(im.urls.iter().map(|u| make_img(u)));
    }
    out.push(json!({"role": "user", "content": parts}));
}

pub(crate) fn active_turn_start(msgs: &[Message]) -> usize {
    msgs.iter()
        .rposition(|m| matches!(m, Message::User { .. }))
        .map_or(0, |i| i + 1)
}

pub(crate) fn make_replay(p: String, api: ApiType, m: String, items: Vec<Value>) -> Option<Replay> {
    (!items.is_empty()).then(|| Replay {
        provider: p,
        api: api.as_str().into(),
        model: m,
        items,
    })
}

pub(crate) fn finalize_calls(calls: &mut Vec<ToolCall>) {
    calls.retain(|c| !c.name.is_empty());
    for (i, c) in calls.iter_mut().enumerate() {
        if c.id.is_empty() {
            c.id = format!("call_{i}");
        }
    }
}
