#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::{call, event, spawn};
use tuse_core::{Mode, Options, Session, Start};

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
    let (dir, cfg) = common::setup("rpc-protocol", &addr, "");
    let session = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg);
        }
        Options {
            cwd: dir.clone(),
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
            flags: Default::default(),
        }
    })
    .unwrap();
    let (tx, out, server) = spawn(session);

    let r = call(&tx, &out, 1, "initialize", json!({"protocolVersion": 1}));
    assert_eq!(r["error"]["code"], -32602, "{r}");
    let r = call(&tx, &out, 2, "session.state", json!({}));
    assert_eq!(
        r["error"]["code"], -32002,
        "not ready after a rejected handshake"
    );
    let r = call(&tx, &out, 3, "initialize", json!({}));
    assert_eq!(r["result"]["protocolVersion"], 2, "{r}");

    for (i, method) in [
        "config.set",
        "session.set",
        "session.records",
        "session.trace",
        "session.info",
        "session.stats",
        "session.snapshot",
        "session.resume",
        "model.get",
    ]
    .into_iter()
    .enumerate()
    {
        let r = call(&tx, &out, 20 + i as u64, method, json!({}));
        assert_eq!(r["error"]["code"], -32601, "{r}");
    }
    let r = call(
        &tx,
        &out,
        6,
        "model.set",
        json!({"model": "p/m", "effort": "high"}),
    );
    assert_eq!(r["result"], json!(null), "{r}");
    let r = call(&tx, &out, 7, "session.state", json!({}));
    assert_eq!(
        (r["result"]["model"].clone(), r["result"]["effort"].clone()),
        (json!("p/m"), json!("high"))
    );
    assert_eq!(tuse_core::Prefs::effort(), "high");

    let p = json!({"model": "p/cfg", "effort": "low", "persist": false});
    assert_eq!(call(&tx, &out, 8, "model.set", p)["result"], json!(null));
    let r = call(&tx, &out, 9, "session.state", json!({}));
    assert_eq!(
        (r["result"]["model"].clone(), r["result"]["effort"].clone()),
        (json!("p/cfg"), json!("low"))
    );
    assert_eq!(
        tuse_core::Prefs::model(),
        "p/m",
        "persist: false keeps the default"
    );
    assert_eq!(
        call(
            &tx,
            &out,
            40,
            "tasks.get",
            json!({"id": "t1", "lines": "bad"})
        )["error"]["code"],
        -32602
    );
    assert_eq!(
        call(&tx, &out, 41, "session.list", json!({"cwd": 123}))["error"]["code"],
        -32602
    );
    assert_eq!(
        call(
            &tx,
            &out,
            42,
            "turn.submit",
            json!({"text": "x", "images": ["data:image/png;base64,not!base"]})
        )["error"]["code"],
        -32000
    );

    let r = call(&tx, &out, 10, "model.list", json!({"cached": true}));
    assert_eq!(r["result"], json!([]));
    assert_eq!(hits.load(Ordering::SeqCst), 0, "cached reads never fetch");
    let r = call(&tx, &out, 11, "model.list", json!({}));
    assert_eq!(r["result"], json!([]), "a snapshot, not the finished fetch");
    event(&out, "models");
    let r = call(&tx, &out, 12, "model.list", json!({"cached": true}));
    assert_eq!(r["result"], json!(["p/remote"]));
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
