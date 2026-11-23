//! A whole `tuse json` session against a fake provider.
#[path = "../../core/tests/common/mod.rs"]
mod common;
use common::Reply;
use serde_json::{Value, json};
use std::io::BufReader;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use tuse_core::{Mode, Options, Session, Start};

mod support;
use support::{Lines, Pipe, event, reply, until};

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
    let send = |v: Value| req_tx.send(format!("{v}\n")).unwrap();
    let call = |id: u64, method: &str, params: Value| {
        send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
    };

    call(1, "session.info", json!({}));
    assert_eq!(reply(&out, 1)["error"]["code"], -32002);
    req_tx.send("{not json\n".into()).unwrap();
    assert_eq!(
        until(&out, |v| v.get("error").is_some())["error"]["code"],
        -32700
    );
    call(
        2,
        "initialize",
        json!({"protocolVersion": 1, "client": {"name": "t"}}),
    );
    let init = reply(&out, 2)["result"].clone();
    assert_eq!(init["protocolVersion"], 1);
    assert_eq!(init["model"], "p/m");
    assert!(init["capabilities"]["methods"].as_array().unwrap().len() > 10);
    call(3, "nope", json!({}));
    assert_eq!(reply(&out, 3)["error"]["code"], -32601);

    call(4, "turn.submit", json!({"text": "deploy it"}));
    assert_eq!(reply(&out, 4)["result"]["queued"], false);
    assert_eq!(event(&out, "user")["text"], "deploy it");
    let end = event(&out, "tool_end");
    assert!(
        end["output"].as_str().unwrap().contains("answer: yes"),
        "{end}"
    );
    assert_eq!(event(&out, "assistant")["text"], "deployed");
    assert_eq!(event(&out, "turn_end")["cancelled"], false);

    call(6, "session.stats", json!({}));
    let stats = reply(&out, 6)["result"].clone();
    assert_eq!(
        (stats["requests"].clone(), stats["turns"].clone()),
        (json!(2), json!(1))
    );
    call(61, "session.snapshot", json!({}));
    let snap = reply(&out, 61)["result"].clone();
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
    assert!(snap["trace"].as_array().unwrap().len() >= 4);
    assert_eq!(snap["ctx_size"], 50000);
    call(62, "config.get", json!({}));
    assert_eq!(reply(&out, 62)["result"]["models"][0]["display"], "M");
    call(7, "session.records", json!({}));
    let records = reply(&out, 7)["result"].clone();
    assert!(records.as_array().unwrap().iter().any(|r| r["t"] == "tool"));
    call(8, "tasks.get", json!({"id": "t9"}));
    assert_eq!(reply(&out, 8)["error"]["code"], -32000);
    call(9, "shutdown", json!({}));
    assert_eq!(reply(&out, 9)["result"], Value::Null);
    server.join().unwrap();
    common::cleanup(&dir);
}
