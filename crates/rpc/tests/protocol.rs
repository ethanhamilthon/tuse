//! The `initialize` handshake, model setters and the `model.list` refresh flow.
#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::{call, event, spawn};
use tuse_core::{Mode, Options, Session, Start};

/// A `/models` endpoint that counts its requests.
fn models_server(hits: Arc<AtomicUsize>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.trim().is_empty() {
                line.clear();
            }
            hits.fetch_add(1, Ordering::SeqCst);
            let body = r#"{"data":[{"id":"remote"}]}"#;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    addr
}

#[test]
fn handshake_setters_and_model_refresh() {
    let hits = Arc::new(AtomicUsize::new(0));
    let addr = models_server(hits.clone());
    let (dir, cfg) = common::setup("rpc-protocol", &addr, "models = {{ name = 'p/cfg' }}");
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let (tx, out, server) = spawn(session);

    let r = call(&tx, &out, 1, "initialize", json!({"protocolVersion": 2}));
    assert_eq!(r["error"]["code"], -32602, "{r}");
    let r = call(&tx, &out, 2, "session.info", json!({}));
    assert_eq!(
        r["error"]["code"], -32002,
        "not ready after a rejected handshake"
    );
    let r = call(&tx, &out, 3, "initialize", json!({}));
    assert_eq!(r["result"]["protocolVersion"], 1, "{r}");

    let r = call(
        &tx,
        &out,
        4,
        "config.set",
        json!({"key": "effort", "value": "huge"}),
    );
    assert_eq!(r["error"]["code"], -32602, "{r}");
    let r = call(
        &tx,
        &out,
        5,
        "config.set",
        json!({"key": "model", "value": "nope/x"}),
    );
    assert_eq!(r["error"]["code"], -32000, "{r}");
    let r = call(
        &tx,
        &out,
        6,
        "config.set",
        json!({"key": "effort", "value": "high"}),
    );
    assert_eq!(r["result"], json!(null), "{r}");
    let r = call(&tx, &out, 7, "model.get", json!({}));
    assert_eq!(r["result"], json!({"model": "p/m", "effort": "high"}));
    assert_eq!(tuse_core::default_effort(), "high");

    let p = json!({"model": "p/cfg", "effort": "low", "persist": false});
    assert_eq!(call(&tx, &out, 8, "model.set", p)["result"], json!(null));
    let r = call(&tx, &out, 9, "model.get", json!({}));
    assert_eq!(r["result"], json!({"model": "p/cfg", "effort": "low"}));
    assert_eq!(
        tuse_core::default_model(),
        "p/m",
        "persist: false keeps the default"
    );

    let r = call(&tx, &out, 10, "model.list", json!({"cached": true}));
    assert_eq!(r["result"], json!(["p/cfg"]));
    assert_eq!(hits.load(Ordering::SeqCst), 0, "cached reads never fetch");
    let r = call(&tx, &out, 11, "model.list", json!({}));
    assert_eq!(
        r["result"],
        json!(["p/cfg"]),
        "a snapshot, not the finished fetch"
    );
    event(&out, "models");
    let r = call(&tx, &out, 12, "model.list", json!({"cached": true}));
    assert_eq!(r["result"], json!(["p/cfg", "p/remote"]));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "one fetch for the whole flow"
    );
    let r = call(&tx, &out, 13, "model.list", json!({"cached": 1}));
    assert_eq!(r["error"]["code"], -32602);

    call(&tx, &out, 14, "shutdown", json!({}));
    server.join().unwrap();
    common::cleanup(&dir);
}
