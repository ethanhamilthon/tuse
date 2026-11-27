use crate::http::Conn;
use crate::types::{ApiType, Delta, Error, Reply, Request};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const RETRY_DELAYS: [u64; 5] = [2, 5, 10, 20, 30];

pub struct Client {
    api: ApiType,
    conn: Conn,
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
    hdrs: &[(&str, &str)],
    timeout: Duration,
) -> Result<(u16, String), String> {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .build();
    let req = hdrs.iter().fold(agent.get(url), |r, (k, v)| r.set(k, v));
    match req.call() {
        Ok(r) => Ok((r.status(), r.into_string().unwrap_or_default())),
        Err(ureq::Error::Status(s, r)) => Ok((s, r.into_string().unwrap_or_default())),
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
            api,
            conn: Conn::new(agent, base_url, api_key),
        }
    }

    pub fn with_headers(mut self, headers: std::collections::BTreeMap<String, String>) -> Self {
        self.conn.set_headers(headers);
        self
    }

    pub fn complete(
        &self,
        req: &Request,
        retries: usize,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
        on_retry: &mut dyn FnMut(Retry),
    ) -> Result<Reply, Error> {
        self.complete_observed(req, retries, cancel, on_delta, on_retry, &mut || {})
    }

    pub fn complete_observed(
        &self,
        req: &Request,
        retries: usize,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
        on_retry: &mut dyn FnMut(Retry),
        on_attempt: &mut dyn FnMut(),
    ) -> Result<Reply, Error> {
        self.complete_with_delays(
            req,
            &RETRY_DELAYS[..retries.min(RETRY_DELAYS.len())],
            cancel,
            on_delta,
            on_retry,
            on_attempt,
        )
    }

    fn complete_with_delays(
        &self,
        req: &Request,
        delays: &[u64],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
        on_retry: &mut dyn FnMut(Retry),
        on_attempt: &mut dyn FnMut(),
    ) -> Result<Reply, Error> {
        let (mut attempt, max) = (0, delays.len());
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            on_attempt();
            match self
                .once(req, cancel, on_delta)
                .map_err(|e| self.conn.redact(e))
            {
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
        match self.api {
            ApiType::OpenAiChat => crate::openai_chat::complete(&self.conn, req, cancel, on_delta),
            ApiType::OpenAiResponses => {
                crate::openai_responses::complete(&self.conn, req, cancel, on_delta)
            }
            ApiType::AnthropicMessages => {
                crate::anthropic::complete(&self.conn, req, cancel, on_delta)
            }
            ApiType::GoogleGemini => crate::gemini::complete(&self.conn, req, cancel, on_delta),
        }
    }

    pub fn models(&self) -> Result<Vec<String>, Error> {
        match self.api {
            ApiType::OpenAiChat | ApiType::OpenAiResponses => self.conn.openai_models(),
            ApiType::AnthropicMessages => crate::anthropic::models(&self.conn),
            ApiType::GoogleGemini => crate::gemini::models(&self.conn),
        }
        .map_err(|e| self.conn.redact(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, atomic::AtomicUsize};

    fn mock(resps: Vec<String>) -> (String, Arc<AtomicUsize>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let Some(resp) = resps.get(h.fetch_add(1, Ordering::SeqCst)) else {
                    break;
                };
                let (mut r, mut len) = (BufReader::new(s.try_clone().unwrap()), 0usize);
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
                    let _ = r.read_exact(&mut vec![0; len]);
                }
                if resp != "DROP" {
                    let _ = s.write_all(resp.as_bytes());
                }
            }
        });
        (url, hits)
    }

    #[test]
    fn retries_only_retryable_errors() {
        let cancel = AtomicBool::new(false);
        let msg = vec![Message::User {
            text: "hi".into(),
            images: vec![],
        }];
        let req = Request {
            provider: "p".into(),
            model: "m".into(),
            system: "".into(),
            messages: msg,
            tools: vec![],
            effort: None,
        };
        let ok = |ct: &str, b: &str| {
            format!("HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nConnection: close\r\n\r\n{b}")
        };
        let st = |c: u16| format!("HTTP/1.1 {c} X\r\nConnection: close\r\n\r\ne");
        let sse = |b: &str| ok("text/event-stream", &format!("data: {b}\n\n"));
        let js = |b: &str| ok("application/json", b);
        for (api, resps, wn, wt) in [
            (
                ApiType::OpenAiChat,
                vec![
                    st(429),
                    st(503),
                    "DROP".into(),
                    js(r#"{"choices":[{"message":{"content":"ok"}}]}"#),
                ],
                3,
                "ok",
            ),
            (
                ApiType::AnthropicMessages,
                vec![
                    sse(r#"{"type":"error","error":{"type":"overloaded_error"}}"#),
                    js(r#"{"content":[{"type":"text","text":"a"}],"stop_reason":"end_turn"}"#),
                ],
                1,
                "a",
            ),
            (
                ApiType::OpenAiResponses,
                vec![
                    sse(
                        r#"{"type":"response.failed","response":{"error":{"code":"rate_limit_exceeded"}}}"#,
                    ),
                    js(
                        r#"{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"r"}]}]}"#,
                    ),
                ],
                1,
                "r",
            ),
        ] {
            let (url, hits) = mock(resps);
            let mut n = 0;
            let r = Client::new(&url, "k", api)
                .complete_with_delays(
                    &req,
                    &[0; 5],
                    &cancel,
                    &mut |_| {},
                    &mut |_| n += 1,
                    &mut || {},
                )
                .unwrap();
            assert!(r.text == wt && n == wn && hits.load(Ordering::SeqCst) == wn + 1);
        }
        for (api, resp) in [
            (ApiType::OpenAiChat, st(400)),
            (
                ApiType::AnthropicMessages,
                sse(r#"{"type":"error","error":{"type":"invalid_request_error"}}"#),
            ),
            (
                ApiType::OpenAiResponses,
                sse(
                    r#"{"type":"response.failed","response":{"error":{"code":"context_length_exceeded"}}}"#,
                ),
            ),
            (ApiType::GoogleGemini, js("bad")),
        ] {
            let (url, hits) = mock(vec![resp.clone(), resp]);
            let mut n = 0;
            let e = Client::new(&url, "k", api)
                .complete_with_delays(
                    &req,
                    &[0; 2],
                    &cancel,
                    &mut |_| {},
                    &mut |_| n += 1,
                    &mut || {},
                )
                .unwrap_err();
            assert!(!e.retryable() && n == 0 && hits.load(Ordering::SeqCst) == 1);
        }
    }
}
