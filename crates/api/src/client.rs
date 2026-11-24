use crate::types::{ApiType, Delta, Error, Reply, Request, ToolCall, one_line};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const RETRY_DELAYS: [u64; 5] = [2, 5, 10, 20, 30];

pub struct Client {
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) api: ApiType,
    pub(crate) agent: ureq::Agent,
}

pub struct Retry<'a> {
    pub attempt: usize,
    pub max: usize,
    pub delay: u64,
    pub error: &'a Error,
}

pub fn default_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(600))
        .build()
}

pub fn probe_get(
    url: &str,
    headers: &[(&str, &str)],
    timeout: Duration,
) -> Result<(u16, String), String> {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .build();
    let mut req = agent.get(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    match req.call() {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.into_string().unwrap_or_default();
            Ok((status, body))
        }
        Err(ureq::Error::Status(status, resp)) => {
            Ok((status, resp.into_string().unwrap_or_default()))
        }
        Err(ureq::Error::Transport(t)) => Err(t.to_string()),
    }
}

pub fn download_to_file(url: &str, dst: &std::path::Path, timeout: Duration) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .build();
    let resp = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    let mut file = std::fs::File::create(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    std::io::copy(&mut resp.into_reader(), &mut file)
        .map_err(|e| format!("{}: {e}", dst.display()))?;
    Ok(())
}

impl Client {
    pub fn new(base_url: &str, api_key: &str, api: ApiType) -> Self {
        Self::with_agent(default_agent(), base_url, api_key, api)
    }

    pub fn with_agent(agent: ureq::Agent, base_url: &str, api_key: &str, api: ApiType) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            api,
            agent,
        }
    }

    pub fn complete(
        &self,
        req: &Request,
        retries: usize,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
        on_retry: &mut dyn FnMut(Retry),
    ) -> Result<Reply, Error> {
        self.complete_with_delays(req, retries, &RETRY_DELAYS, cancel, on_delta, on_retry)
    }

    pub(crate) fn complete_with_delays(
        &self,
        req: &Request,
        retries: usize,
        delays: &[u64],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
        on_retry: &mut dyn FnMut(Retry),
    ) -> Result<Reply, Error> {
        let mut attempt = 0;
        let max = retries.min(delays.len());
        loop {
            match self.once(req, cancel, on_delta) {
                Err(e) if e.retryable() && attempt < max => {
                    let delay = delays[attempt];
                    attempt += 1;
                    on_retry(Retry {
                        attempt,
                        max,
                        delay,
                        error: &e,
                    });
                    for _ in 0..delay * 10 {
                        if cancel.load(Ordering::Relaxed) {
                            return Err(Error::Cancelled);
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                r => return r,
            }
        }
    }

    fn once(
        &self,
        req: &Request,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
    ) -> Result<Reply, Error> {
        validate_effort(self.api, req)?;
        match self.api {
            ApiType::OpenAiChat => crate::openai_chat::complete(self, req, cancel, on_delta),
            ApiType::OpenAiResponses => {
                crate::openai_responses::complete(self, req, cancel, on_delta)
            }
            ApiType::AnthropicMessages => crate::anthropic::complete(self, req, cancel, on_delta),
            ApiType::GoogleGemini => crate::gemini::complete(self, req, cancel, on_delta),
        }
    }

    pub fn models(&self) -> Result<Vec<String>, Error> {
        match self.api {
            ApiType::OpenAiChat => crate::openai_chat::models(self),
            ApiType::OpenAiResponses => crate::openai_responses::models(self),
            ApiType::AnthropicMessages => crate::anthropic::models(self),
            ApiType::GoogleGemini => crate::gemini::models(self),
        }
    }
}

fn validate_effort(api: ApiType, req: &Request) -> Result<(), Error> {
    let Some(effort) = req.effort.as_deref() else {
        return Ok(());
    };
    if api.supports_effort(effort) {
        return Ok(());
    }
    Err(Error::Unsupported(format!(
        "provider `{}` ({}) does not support effort `{effort}` (supported: {})",
        req.provider,
        api.as_str(),
        api.supported_efforts().join(", ")
    )))
}

pub(crate) fn check_status(resp: ureq::Response) -> Result<ureq::Response, Error> {
    let code = resp.status();
    if (300..400).contains(&code) {
        let loc = resp.header("location").unwrap_or("(unknown)");
        return Err(Error::Status(code, format!("unexpected redirect to {loc}")));
    }
    Ok(resp)
}

pub(crate) fn parse_data_url<'a>(
    url: &'a str,
    provider: &str,
    api: ApiType,
) -> Result<(&'a str, &'a str), Error> {
    let fail = || {
        Error::Unsupported(format!(
            "provider `{provider}` ({}) does not support image `{}` (requires base64 data:<mime>;base64,<data> URL)",
            api.as_str(),
            one_line(url),
        ))
    };
    let rest = url.strip_prefix("data:").ok_or_else(fail)?;
    let (mime, data) = rest.split_once(";base64,").ok_or_else(fail)?;
    if mime.is_empty()
        || !mime.contains('/')
        || mime.chars().any(char::is_whitespace)
        || !valid_base64(data)
    {
        return Err(fail());
    }
    Ok((mime, data))
}

fn valid_base64(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return false;
    }
    let pad = bytes.iter().rev().take_while(|&&b| b == b'=').count();
    if pad > 2 {
        return false;
    }
    let body = &bytes[..bytes.len() - pad];
    if body.is_empty() || body.iter().any(|&b| b64_val(b).is_none()) {
        return false;
    }
    match pad {
        1 => b64_val(body[body.len() - 1]).is_some_and(|v| v & 0x03 == 0),
        2 => b64_val(body[body.len() - 1]).is_some_and(|v| v & 0x0f == 0),
        _ => true,
    }
}

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub(crate) fn parse_json_object(raw: &str) -> Value {
    match serde_json::from_str::<Value>(raw) {
        Ok(v) if v.is_object() => v,
        _ => json!({}),
    }
}

pub(crate) fn read_sse(
    reader: impl Read,
    cancel: &AtomicBool,
    mut on_event: impl FnMut(&str, &str) -> Result<bool, Error>,
) -> Result<bool, Error> {
    let mut event = String::new();
    for line in BufReader::new(reader).lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let line = line.map_err(|e| Error::Transport(e.to_string()))?;
        if line.is_empty() {
            event.clear();
            continue;
        }
        if let Some(ev) = line.strip_prefix("event:") {
            event = ev.trim().to_string();
            continue;
        }
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            return Ok(true);
        }
        if data.is_empty() {
            continue;
        }
        if !on_event(&event, data)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn read_body(resp: ureq::Response) -> Result<String, Error> {
    let mut text = String::new();
    resp.into_reader()
        .read_to_string(&mut text)
        .map_err(|e| Error::Transport(e.to_string()))?;
    Ok(text)
}

pub(crate) fn parse_json(text: &str) -> Result<Value, Error> {
    serde_json::from_str(text).map_err(|e| Error::Parse(e.to_string()))
}

pub(crate) fn finalize_calls(calls: &mut Vec<ToolCall>) {
    calls.retain(|c| !c.name.is_empty());
    for (i, c) in calls.iter_mut().enumerate() {
        if c.id.is_empty() {
            c.id = format!("call_{i}");
        }
    }
}

pub(crate) fn map_err(e: ureq::Error) -> Error {
    match e {
        ureq::Error::Status(code, resp) => {
            Error::Status(code, resp.into_string().unwrap_or_default())
        }
        ureq::Error::Transport(t) => Error::Transport(t.to_string()),
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

pub(crate) fn openai_models(client: &Client) -> Result<Vec<String>, Error> {
    let resp = check_status(
        client
            .agent
            .get(&format!("{}/models", client.base_url))
            .set("Authorization", &format!("Bearer {}", client.api_key))
            .call()
            .map_err(map_err)?,
    )?;
    let v = parse_json(&read_body(resp)?)?;
    Ok(v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().map(String::from))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    fn mock(responses: Vec<String>) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let n = h.fetch_add(1, Ordering::SeqCst);
                let Some(resp) = responses.get(n).cloned() else {
                    break;
                };
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                if len > 0 {
                    let mut buf = vec![0; len];
                    let _ = r.read_exact(&mut buf);
                }
                let mut s = stream;
                if resp == "DROP" {
                    drop(s);
                } else {
                    let _ = s.write_all(resp.as_bytes());
                }
            }
        });
        (addr, hits)
    }

    fn simple_req() -> Request {
        Request {
            provider: "p".into(),
            model: "m".into(),
            system: String::new(),
            messages: vec![Message::User {
                text: "hi".into(),
                images: vec![],
            }],
            tools: vec![],
            effort: None,
        }
    }

    #[test]
    fn base64_data_url_validation_rejects_malformed_payloads() {
        for good in [
            "data:image/png;base64,QUFB",
            "data:image/jpeg;base64,AA==",
            "data:image/webp;base64,AAA=",
        ] {
            assert!(
                parse_data_url(good, "p", ApiType::AnthropicMessages).is_ok(),
                "{good}"
            );
        }
        for bad in [
            "https://example.com/a.png",
            "data:image/png;base64,",
            "data:image/png;base64,ABC",
            "data:image/png;base64,not!base",
            "data:image/png;base64,A===",
            "data:image/png;base64,AB==",
            "data:image/png;base64,AAB=",
            "data:image/png;base64,QU=B",
        ] {
            assert!(
                matches!(
                    parse_data_url(bad, "p", ApiType::GoogleGemini),
                    Err(Error::Unsupported(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn redirects_are_blocked_without_leaking_credentials() {
        let (sink_url, sink_hits) = mock(vec![
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}"
                .into();
            16
        ]);
        let redir = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {sink_url}/steal\r\nConnection: close\r\n\r\n"
        );
        let (front_url, _) = mock(vec![redir; 16]);
        let cancel = AtomicBool::new(false);
        for api in [
            ApiType::OpenAiChat,
            ApiType::OpenAiResponses,
            ApiType::AnthropicMessages,
            ApiType::GoogleGemini,
        ] {
            let client = Client::new(&front_url, "secret-key", api);
            let m_err = client.models().unwrap_err();
            assert!(matches!(m_err, Error::Status(307, _)), "{api:?}: {m_err}");
            assert!(!m_err.retryable());
            let c_err = client
                .complete(&simple_req(), 0, &cancel, &mut |_| {}, &mut |_| {})
                .unwrap_err();
            assert!(matches!(c_err, Error::Status(307, _)), "{api:?}: {c_err}");
            assert!(!c_err.retryable());
        }
        assert_eq!(sink_hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn truncated_sse_streams_fail_as_retryable_transport_errors() {
        let cases = [
            (
                ApiType::OpenAiChat,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"lua\",\"arguments\":\"{\\\"code\\\":\\\"x()\\\"}\"}}]}}]}\n\n",
            ),
            (
                ApiType::OpenAiResponses,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"fc_1\",\"type\":\"function_call\",\"call_id\":\"c1\",\"name\":\"lua\",\"arguments\":\"{\\\"code\\\":\\\"x()\\\"}\"}}\n\n",
            ),
            (
                ApiType::AnthropicMessages,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu_1\",\"name\":\"lua\",\"input\":{}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"code\\\":\\\"x()\\\"}\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            ),
            (
                ApiType::GoogleGemini,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"lua\",\"args\":{\"code\":\"x()\"}}}]}}]}\n\n",
            ),
        ];
        let cancel = AtomicBool::new(false);
        for (api, raw) in cases {
            let (url, _) = mock(vec![raw.into()]);
            let err = Client::new(&url, "k", api)
                .complete(&simple_req(), 0, &cancel, &mut |_| {}, &mut |_| {})
                .unwrap_err();
            assert!(
                matches!(err, Error::Transport(_)) && err.retryable(),
                "{api:?}: {err}"
            );
        }
    }

    #[test]
    fn retries_on_429_5xx_transport_and_inline_retryable_errors_only() {
        let ok_resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"choices\":[{\"message\":{\"content\":\"ok\"}}]}".to_string();
        let (url, hits) = mock(vec![
            "HTTP/1.1 429 Too Many Requests\r\nConnection: close\r\n\r\nslow down".into(),
            "HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\n\r\nbusy".into(),
            "DROP".into(),
            ok_resp,
        ]);
        let client = Client::new(&url, "k", ApiType::OpenAiChat);
        let cancel = AtomicBool::new(false);
        let mut retries = 0;
        let reply = client
            .complete_with_delays(
                &simple_req(),
                5,
                &[0, 0, 0, 0, 0],
                &cancel,
                &mut |_| {},
                &mut |_| retries += 1,
            )
            .unwrap();
        assert_eq!(reply.text, "ok");
        assert_eq!((retries, hits.load(Ordering::SeqCst)), (3, 4));

        let (url, hits) = mock(vec![
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"busy\"}}\n\n".into(),
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\",\"message\":\"slow\"}}\n\n".into(),
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"content\":[{\"type\":\"text\",\"text\":\"recovered\"}],\"stop_reason\":\"end_turn\"}".into(),
        ]);
        let client = Client::new(&url, "k", ApiType::AnthropicMessages);
        let mut retries = 0;
        let reply = client
            .complete_with_delays(
                &simple_req(),
                5,
                &[0, 0, 0],
                &cancel,
                &mut |_| {},
                &mut |_| retries += 1,
            )
            .unwrap();
        assert_eq!(reply.text, "recovered");
        assert_eq!((retries, hits.load(Ordering::SeqCst)), (2, 3));

        let (url, hits) = mock(vec![
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"rate_limit_exceeded\",\"message\":\"wait\"}}}\n\n".into(),
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"server_error\",\"message\":\"oops\"}}}\n\n".into(),
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"done\"}]}]}".into(),
        ]);
        let client = Client::new(&url, "k", ApiType::OpenAiResponses);
        let mut retries = 0;
        let reply = client
            .complete_with_delays(
                &simple_req(),
                5,
                &[0, 0, 0],
                &cancel,
                &mut |_| {},
                &mut |_| retries += 1,
            )
            .unwrap();
        assert_eq!(reply.text, "done");
        assert_eq!((retries, hits.load(Ordering::SeqCst)), (2, 3));

        for (api, resp) in [
            (
                ApiType::OpenAiChat,
                "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\nbad",
            ),
            (
                ApiType::AnthropicMessages,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"bad\"}}\n\n",
            ),
            (
                ApiType::OpenAiResponses,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"context_length_exceeded\",\"message\":\"long\"}}}\n\n",
            ),
            (
                ApiType::GoogleGemini,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\nnot-json",
            ),
        ] {
            let (url, hits) = mock(vec![resp.into(), resp.into()]);
            let client = Client::new(&url, "k", api);
            let mut retries = 0;
            let err = client
                .complete_with_delays(&simple_req(), 5, &[0, 0], &cancel, &mut |_| {}, &mut |_| {
                    retries += 1
                })
                .unwrap_err();
            assert!(!err.retryable(), "{api:?}: {err}");
            assert_eq!((retries, hits.load(Ordering::SeqCst)), (0, 1), "{api:?}");
        }
    }
}
