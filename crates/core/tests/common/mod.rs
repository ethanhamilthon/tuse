//! A fake Chat Completions provider for the integration tests.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

/// What the provider answers to one request.
#[derive(Clone)]
pub enum Reply {
    /// A tool call: name and JSON arguments, streamed in small pieces.
    Call(&'static str, String),
    Text(&'static str),
    /// Streams the text, then holds the response open until the client goes away.
    Stall(&'static str),
}

/// Serves `replies` in order (the last one repeats) and returns `host:port`.
pub fn provider(replies: Vec<Reply>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for (n, stream) in listener.incoming().flatten().enumerate() {
            let reply = replies[n.min(replies.len() - 1)].clone();
            std::thread::spawn(move || answer(stream, reply));
        }
    });
    addr
}

fn answer(stream: std::net::TcpStream, reply: Reply) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut len = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.trim().is_empty() {
            break;
        }
        if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).unwrap();
    let mut chunks: Vec<Value> = vec![];
    match reply {
        Reply::Call(name, args) => {
            for (i, part) in args.as_bytes().chunks(17).enumerate() {
                let mut f = json!({"arguments": String::from_utf8_lossy(part)});
                if i == 0 {
                    f["name"] = json!(name);
                }
                chunks.push(json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":format!("c{len}"),"function":f}]}}]}));
            }
        }
        Reply::Text(t) => chunks.push(json!({"choices":[{"delta":{"content":t}}]})),
        Reply::Stall(t) => {
            let mut out = stream;
            let _ = out.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            let c = json!({"choices":[{"delta":{"content":t}}]});
            let _ = write!(out, "data: {c}\n\n");
            let _ = out.flush();
            while out.write_all(b": ping\n\n").is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            return;
        }
    }
    chunks.push(json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":3}}));
    let mut out = stream;
    let _ = out.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
    );
    for c in chunks {
        let _ = write!(out, "data: {c}\n\n");
    }
    let _ = out.write_all(b"data: [DONE]\n\n");
}

/// A temporary folder with `TUSE_HOME` inside it, provider `p` at `addr` and a
/// `tuse.lua` whose table has `extra` (Lua fields) besides the model.
/// Each test file holds one test, so setting the environment is safe.
pub fn setup(name: &str, addr: &str, extra: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tuse-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: see above.
    unsafe {
        std::env::set_var("TUSE_HOME", dir.join("home"));
    }
    tuse_core::add_provider(&tuse_config::Connect {
        name: "p".into(),
        base_url: format!("http://{addr}"),
        api_key: "k".into(),
    })
    .unwrap();
    let cfg = dir.join("tuse.lua");
    std::fs::write(&cfg, format!("return {{\n  model = 'p/m',\n{extra}\n}}\n")).unwrap();
    (dir, cfg)
}

pub fn cleanup(dir: &Path) {
    std::fs::remove_dir_all(dir).ok();
}
