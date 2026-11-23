//! JSON-RPC parity: Lua UI, custom commands, prompt refs, model persistence, session new/resume.
#[path = "../../core/tests/common/mod.rs"]
mod common;
#[allow(dead_code)]
mod support;

use common::Reply;
use serde_json::json;
use support::{call, event, spawn, until};
use tuse_core::{Mode, Options, Session, Start};

#[test]
fn rpc_ui_commands_prompts_model_and_session_switching() {
    let addr = common::provider(vec![Reply::Text("rpc reply 1")]);
    let extra = r#"
  prompts = { review = "Check edge cases." },
  commands = {
    ui_demo = function()
      local t = require("tuse")
      t.show_markdown("m1", "Hello", true)
      t.show_markdown("drop", "Temp", false)
      t.show_markdown("drop", "", false)
      local p = t.show_picker({"a", "b"})
      local q = t.show_question("Why?")
      local c = t.show_picker({"x"})
      t.show_markdown("done", tostring(p) .. ":" .. tostring(q) .. ":" .. tostring(c), false)
    end,
  },
"#;
    let (dir, cfg) = common::setup("rpc-parity", &addr, extra);
    let s1 = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let s1_id = s1.id.clone();
    let (tx, out, server) = spawn(s1);
    assert_eq!(
        call(&tx, &out, 1, "initialize", json!({}))["result"]["session"]["id"],
        s1_id
    );
    assert_eq!(
        call(&tx, &out, 2, "commands.list", json!({}))["result"],
        json!(["ui_demo"])
    );

    // Custom command driving show_markdown, show_picker, show_question.
    assert!(call(&tx, &out, 3, "commands.run", json!({"name": "ui_demo"}))["error"].is_null());
    let r1 = event(&out, "ui_request");
    assert_eq!(r1["kind"], "picker");
    assert!(
        call(
            &tx,
            &out,
            4,
            "ui.reply",
            json!({"id": r1["id"], "value": "b"})
        )["error"]
            .is_null()
    );
    assert_eq!(event(&out, "ui_result")["value"], "b");
    let r2 = event(&out, "ui_request");
    assert_eq!(r2["kind"], "question");
    assert!(
        call(
            &tx,
            &out,
            5,
            "ui.reply",
            json!({"id": r2["id"], "value": "because"})
        )["error"]
            .is_null()
    );
    assert_eq!(event(&out, "ui_result")["value"], "because");
    let r3 = event(&out, "ui_request");
    assert!(call(&tx, &out, 6, "ui.cancel", json!({"id": r3["id"]}))["error"].is_null());
    let done = until(&out, |v| {
        v["method"] == "event"
            && v["params"]["type"] == "ui_markdown"
            && v["params"]["name"] == "done"
    })["params"]
        .clone();
    assert_eq!(done["text"], "b:because:nil");

    // Reload keeps transient markdowns (with pin state) and UI entries.
    assert!(call(&tx, &out, 7, "config.reload", json!({}))["error"].is_null());
    let st = call(&tx, &out, 8, "ui.state", json!({}))["result"].clone();
    assert_eq!(st["entries"].as_array().unwrap().len(), 2);
    assert!(
        st["markdowns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["name"] == "m1" && m["pin"] == true)
    );
    assert!(
        st["markdowns"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["name"] != "drop")
    );

    // Prompt references + single-call model/effort selection & persistence.
    assert_eq!(
        call(
            &tx,
            &out,
            9,
            "turn.submit",
            json!({"text": "hi", "prompts": ["missing"]})
        )["error"]["code"],
        -32000
    );
    assert!(
        call(
            &tx,
            &out,
            10,
            "model.set",
            json!({"model": "p/alt", "effort": "high"})
        )["error"]
            .is_null()
    );
    assert_eq!(
        call(
            &tx,
            &out,
            11,
            "model.set",
            json!({"model": "bad/m", "effort": "high"})
        )["error"]["code"],
        -32000
    );
    assert!(
        call(
            &tx,
            &out,
            12,
            "turn.submit",
            json!({"text": "check", "prompts": ["review"]})
        )["error"]
            .is_null()
    );
    event(&out, "turn_end");
    let trace = call(&tx, &out, 13, "session.trace", json!({}))["result"].clone();
    assert!(trace.as_array().unwrap().iter().any(|t| {
        t["role"] == "user"
            && t["text"]
                .as_str()
                .unwrap()
                .contains("<prompt name=\"review\">Check edge cases.</prompt>")
    }));

    // In-process session.new inherits persisted model/effort; bad session.resume keeps S2 alive; valid resume returns to S1 without transient UI.
    let s2 = call(&tx, &out, 14, "session.new", json!({}))["result"].clone();
    assert_ne!(s2["session"]["id"], s1_id);
    assert_eq!(
        (s2["model"].clone(), s2["effort"].clone()),
        (json!("p/alt"), json!("high"))
    );
    assert_eq!(
        call(&tx, &out, 15, "session.resume", json!({"id": "nope"}))["error"]["code"],
        -32000
    );
    assert_eq!(
        call(&tx, &out, 16, "session.info", json!({}))["result"]["id"],
        s2["session"]["id"]
    );
    let back = call(&tx, &out, 17, "session.resume", json!({"id": s1_id}))["result"].clone();
    assert_eq!(
        (
            back["session"]["id"].clone(),
            back["model"].clone(),
            back["effort"].clone()
        ),
        (json!(s1_id), json!("p/alt"), json!("high"))
    );
    let rui = call(&tx, &out, 18, "ui.state", json!({}))["result"].clone();
    assert!(
        rui["markdowns"].as_array().unwrap().is_empty()
            && rui["entries"].as_array().unwrap().is_empty()
    );

    call(&tx, &out, 19, "shutdown", json!({}));
    server.join().unwrap();
    common::cleanup(&dir);
}
