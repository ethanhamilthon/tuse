#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use serde_json::json;
use std::fs;
use support::{call, event, spawn};
use tuse_core::{Mode, Options, Session, Start};

#[test]
fn config_reload_accepts_an_optional_namespace() {
    let (dir, config) = common::setup("rpc-reload", "localhost:1", "");
    fs::write(&config, "return {global = false, work = {}, other = {}}").unwrap();
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(config.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let (tx, out, server) = spawn(session);
    let call = |id, method, params| call(&tx, &out, id, method, params);
    call(1, "initialize", json!({}));
    fs::write(
        &config,
        r#"
        return {global = false,
            work = {prompts = {selected = 'new'}},
            other = {prompts = {unselected = 'new'}},
        }
    "#,
    )
    .unwrap();
    for (id, ns, code) in [
        (2, json!(42), -32602),
        (3, json!(""), -32602),
        (4, json!("nope"), -32000),
    ] {
        assert_eq!(
            call(id, "config.reload", json!({"ns": ns}))["error"]["code"],
            code
        );
    }
    assert!(call(5, "config.get", json!({}))["result"]["prompts"]["selected"].is_null());
    assert_eq!(
        call(6, "config.reload", json!({"ns": "work"})).get("result"),
        Some(&json!(null))
    );
    event(&out, "reloaded");
    let scoped = call(7, "config.get", json!({}));
    assert!(!scoped["result"]["prompts"]["selected"].is_null());
    assert!(scoped["result"]["prompts"]["unselected"].is_null());
    assert_eq!(
        call(8, "config.reload", json!({})).get("result"),
        Some(&json!(null))
    );
    assert!(!call(9, "config.get", json!({}))["result"]["prompts"]["unselected"].is_null());
    call(10, "shutdown", json!({}));
    drop(tx);
    server.join().unwrap();
    common::cleanup(&dir);
}
