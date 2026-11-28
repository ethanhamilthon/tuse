#![allow(dead_code)]
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub enum Reply {
    Call(&'static str, String),
    Text(&'static str),
    Stall(&'static str),
}

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

pub fn setup(name: &str, addr: &str, extra: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tuse-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("TUSE_HOME", home.join(".tuse"));
    }
    tuse_core::Providers::add(&tuse_config::Connect {
        name: "p".into(),
        base_url: format!("http://{addr}"),
        api_key: "k".into(),
        ..Default::default()
    })
    .unwrap();
    tuse_core::Prefs::set("model", "p/m").unwrap();
    tuse_core::Prefs::set("effort", "medium").unwrap();
    let cfg = dir.join("tuse.lua");
    let body = if extra.trim().is_empty() {
        "return {}\n".to_string()
    } else {
        format!("return {{\n  test = {{\n{extra}\n  }},\n}}\n")
    };
    std::fs::write(&cfg, body).unwrap();
    (dir, cfg)
}

pub fn cleanup(dir: &Path) {
    std::fs::remove_dir_all(dir).ok();
}
