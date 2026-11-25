#[path = "../../core/tests/common/mod.rs"]
mod common;
use serde_json::json;
use std::time::Duration;
use tuse_core::{Mode, Options, Session, Start};

mod support;
use support::{call, event, spawn};

#[test]
fn startup_errors_arrive_after_the_initialize_answer() {
    let addr = common::provider(vec![]);
    let extra = "  hooks = { broken = { events = {'process_start'}, run = function() error('boom') end } },";
    let (dir, cfg) = common::setup("rpc-early", &addr, extra);
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let (tx, out, server) = spawn(session);
    std::thread::sleep(Duration::from_millis(300));
    tx.send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}).to_string() + "\n")
        .unwrap();
    let first = out.recv_timeout(Duration::from_secs(30)).unwrap();
    assert_eq!(first["id"], 1, "{first}");
    let notice = event(&out, "notice");
    assert!(
        notice["text"].as_str().unwrap().contains("boom"),
        "{notice}"
    );
    call(&tx, &out, 2, "shutdown", json!({}));
    server.join().unwrap();
    common::cleanup(&dir);
}
