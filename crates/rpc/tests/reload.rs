#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use serde_json::json;
use std::fs;
use support::{call, event, spawn};
use tuse_core::{Mode, Options, Session, Start};

#[test]
fn config_reload_rejects_namespace_and_applies_all_changes() {
    let (dir, config) = common::setup("rpc-reload", "localhost:1", "");
    fs::write(&config, "return {work = {}, other = {}}").unwrap();
    unsafe { std::env::set_var("TUSE_GLOBAL_CONFIG", &config) };
    let session = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", config.clone());
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
    let call = |id, method, params| call(&tx, &out, id, method, params);
    call(1, "initialize", json!({}));
    fs::write(
        &config,
        r#"
        return {
            work = {prompts = {selected = 'new'}},
            other = {prompts = {unselected = 'new'}},
        }
    "#,
    )
    .unwrap();
    for (id, ns) in [json!(42), json!(null), json!("work")]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            call(2 + id as u64, "config.reload", json!({"ns": ns}))["error"]["code"],
            -32602,
        );
    }
    assert!(call(5, "config.get", json!({}))["result"]["prompts"]["work:selected"].is_null());
    assert_eq!(
        call(6, "config.reload", json!({})).get("result"),
        Some(&json!(null))
    );
    event(&out, "reloaded");
    let config = call(7, "config.get", json!({}));
    assert!(!config["result"]["prompts"]["work:selected"].is_null());
    assert!(!config["result"]["prompts"]["other:unselected"].is_null());
    call(10, "shutdown", json!({}));
    drop(tx);
    server.join().unwrap();
    common::cleanup(&dir);
}
