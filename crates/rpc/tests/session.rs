#[path = "../../core/tests/common/mod.rs"]
mod common;
use common::Reply;
use serde_json::{Value, json};
use tuse_core::{Mode, Options, Session, Start};

mod support;
use support::{event, spawn, until};

#[test]
fn a_client_drives_a_session() {
    let code = "print('answer: ' .. 'yes')";
    let addr = common::provider(vec![
        Reply::Call("lua", json!({"code": code}).to_string()),
        Reply::Text("deployed"),
    ]);
    let (dir, cfg) = common::setup("rpc", &addr, "");
    std::fs::write(
        &cfg,
        "return { models = {{ name = 'p/m', display = 'M', window = 50000 }} }",
    )
    .unwrap();
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let (req_tx, out, server) = spawn(session);
    let call =
        |id: u64, method: &str, params: Value| support::call(&req_tx, &out, id, method, params);

    assert_eq!(call(1, "session.info", json!({}))["error"]["code"], -32002);
    req_tx.send("{not json\n".into()).unwrap();
    assert_eq!(
        until(&out, |v| v.get("error").is_some())["error"]["code"],
        -32700
    );
    let init = call(
        2,
        "initialize",
        json!({"protocolVersion": 1, "client": {"name": "t"}}),
    )["result"]
        .clone();
    assert_eq!(init["protocolVersion"], 1);
    assert_eq!(init["model"], "p/m");
    assert!(init["capabilities"]["methods"].as_array().unwrap().len() > 10);
    assert_eq!(call(3, "nope", json!({}))["error"]["code"], -32601);

    assert_eq!(
        call(4, "turn.submit", json!({"text": "deploy it"}))["result"]["queued"],
        false
    );
    assert_eq!(event(&out, "user")["text"], "deploy it");
    let end = event(&out, "tool_end");
    assert!(
        end["output"].as_str().unwrap().contains("answer: yes"),
        "{end}"
    );
    assert_eq!(event(&out, "assistant")["text"], "deployed");
    assert_eq!(event(&out, "turn_end")["cancelled"], false);

    let stats = call(6, "session.stats", json!({}))["result"].clone();
    assert_eq!(
        (stats["requests"].clone(), stats["turns"].clone()),
        (json!(2), json!(1))
    );
    let snap = call(61, "session.snapshot", json!({}))["result"].clone();
    assert_eq!(
        (
            snap["mode"].clone(),
            snap["model"].clone(),
            snap["provider"].clone(),
            snap["steps"].clone(),
            snap["turns"].clone()
        ),
        (json!("json"), json!("p/m"), json!("p"), json!(2), json!(1))
    );
    assert!(snap["history"].as_array().unwrap().len() >= 4);
    assert_eq!(snap["ctx_size"], 50000);
    let config = call(62, "config.get", json!({}))["result"].clone();
    assert_eq!(config["models"][0]["display"], "M");
    assert!(config.get("plugins").is_none());
    let records = call(7, "session.history", json!({}))["result"].clone();
    assert!(
        records
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["role"] == "lua")
    );
    assert_eq!(
        call(8, "tasks.get", json!({"id": "t9"}))["error"]["code"],
        -32000
    );
    assert_eq!(call(9, "shutdown", json!({}))["result"], Value::Null);
    server.join().unwrap();
    common::cleanup(&dir);
}
