use crate::stream::Accumulator;
use crate::types::{Delta, Error, Reply, Request};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const RETRY_DELAYS: [u64; 5] = [2, 5, 10, 20, 30];

pub struct Client {
    base_url: String,
    api_key: String,
    agent: ureq::Agent,
}

pub struct Retry<'a> {
    pub attempt: usize,
    pub max: usize,
    pub delay: u64,
    pub error: &'a Error,
}

impl Client {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(30))
            .timeout_read(Duration::from_secs(600))
            .build();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            agent,
        }
    }

    /// Streams a completion, retrying 429, 5xx and network errors up to `retries` times.
    pub fn complete(
        &self,
        req: &Request,
        retries: usize,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
        on_retry: &mut dyn FnMut(Retry),
    ) -> Result<Reply, Error> {
        let mut attempt = 0;
        loop {
            match self.once(req, cancel, on_delta) {
                Err(e) if e.retryable() && attempt < retries.min(RETRY_DELAYS.len()) => {
                    let delay = RETRY_DELAYS[attempt];
                    attempt += 1;
                    on_retry(Retry {
                        attempt,
                        max: retries.min(RETRY_DELAYS.len()),
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
        let mut body = json!({
            "model": req.model,
            "messages": req.messages,
            "stream": true,
            "stream_options": {"include_usage": true},
        });
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(req.tools.clone());
        }
        if let Some(e) = &req.effort {
            body["reasoning_effort"] = json!(e);
        }
        let resp = self
            .agent
            .post(&format!("{}/chat/completions", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(map_err)?;
        let streaming = resp.content_type().contains("event-stream");
        let mut acc = Accumulator::default();
        if !streaming {
            let mut text = String::new();
            resp.into_reader()
                .read_to_string(&mut text)
                .map_err(|e| Error::Transport(e.to_string()))?;
            let v: Value = serde_json::from_str(&text).map_err(|e| Error::Parse(e.to_string()))?;
            check_error(&v)?;
            acc.chunk(&v, on_delta);
            return Ok(acc.finish());
        }
        for line in BufReader::new(resp.into_reader()).lines() {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let line = line.map_err(|e| Error::Transport(e.to_string()))?;
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            let v: Value = serde_json::from_str(data).map_err(|e| Error::Parse(e.to_string()))?;
            check_error(&v)?;
            acc.chunk(&v, on_delta);
        }
        Ok(acc.finish())
    }

    pub fn models(&self) -> Result<Vec<String>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/models", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .call()
            .map_err(map_err)?;
        let text = resp
            .into_string()
            .map_err(|e| Error::Transport(e.to_string()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| Error::Parse(e.to_string()))?;
        Ok(v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["id"].as_str().map(String::from))
            .collect())
    }
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

fn map_err(e: ureq::Error) -> Error {
    match e {
        ureq::Error::Status(code, resp) => {
            Error::Status(code, resp.into_string().unwrap_or_default())
        }
        ureq::Error::Transport(t) => Error::Transport(t.to_string()),
    }
}
