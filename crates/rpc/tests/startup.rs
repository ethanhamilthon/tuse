//! A whole `tuse json` session against a fake provider.
#[path = "../../core/tests/common/mod.rs"]
mod common;
use serde_json::{Value, json};
use std::io::BufReader;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use tuse_core::{Mode, Options, Session, Start};

mod support;
use support::{Lines, Pipe, event, reply};

#[test]
fn startup_errors_arrive_after_the_initialize_answer() {
    let addr = common::provider(vec![]);
    let extra =
        "  hooks = { broken = { events = {'start'}, run = function() error('boom') end } },";
    let (dir, cfg) = common::setup("rpc-early", &addr, extra);
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let (req_tx, req_rx) = mpsc::channel::<String>();
    let (out_tx, out) = mpsc::channel();
    let input = BufReader::new(Pipe(Arc::new(Mutex::new(req_rx)), vec![]));
    let server = std::thread::spawn(move || {
        tuse_rpc::serve(
            session,
            input,
            tuse_rpc::Out::new(Lines(out_tx, vec![])),
            Arc::new(AtomicBool::new(false)),
        )
    });
    std::thread::sleep(Duration::from_millis(300));
    let send = |v: Value| req_tx.send(format!("{v}\n")).unwrap();
    send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
    let first = out.recv_timeout(Duration::from_secs(30)).unwrap();
    assert_eq!(first["id"], 1, "{first}");
    let notice = event(&out, "notice");
    assert!(
        notice["text"].as_str().unwrap().contains("boom"),
        "{notice}"
    );
    send(json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": {}}));
    reply(&out, 2);
    server.join().unwrap();
    common::cleanup(&dir);
}
