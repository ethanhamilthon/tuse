#[path = "../../core/tests/common/mod.rs"]
mod common;
use common::Reply;
use serde_json::json;
use tuse_core::{Mode, Options, Session, Start};

mod support;
use support::{call, event, spawn};

#[test]
fn failed_switch_keeps_the_session() {
    let addr = common::provider(vec![Reply::Text("still here")]);
    let (dir, cfg) = common::setup("rpc-switch", &addr, "");
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let id = session.id.clone();
    let (tx, out, server) = spawn(session);
    call(&tx, &out, 1, "initialize", json!({"protocolVersion": 1}));
    std::fs::write(&cfg, "return { typo_key = 1 }").unwrap();

    let r = call(&tx, &out, 2, "session.new", json!({}));
    let err = r["error"]["message"].as_str().unwrap();
    assert!(err.contains("typo_key"), "{r}");
    let r = call(&tx, &out, 3, "session.info", json!({}));
    assert_eq!(r["result"]["id"], id.as_str(), "{r}");

    call(&tx, &out, 4, "turn.submit", json!({"text": "hi"}));
    assert_eq!(event(&out, "assistant")["text"], "still here");
    assert_eq!(event(&out, "turn_end")["cancelled"], false);
    call(&tx, &out, 5, "shutdown", json!({}));
    server.join().unwrap();
    common::cleanup(&dir);
}
