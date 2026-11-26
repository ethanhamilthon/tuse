#[path = "../../core/tests/common/mod.rs"]
mod common;
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
    deploy = { desc = "ship it", args = { "staging", "prod" }, run = function(env) end },
    rewrite = function()
      local t = require("tuse")
      local s = t.session()
      assert(not pcall(t.set_session, {title = "invalid", history = {{role = "agent", text = "x", tool_calls = {{id = "1", name = "lua", arguments = "{}"}}}}}))
      assert(t.session().name == s.name)
      t.set_session{history = s.history}
      s.history[1].text = "custom system"
      t.set_session{history = s.history}
    end,
    ui_demo = function()
      local t = require("tuse")
      t.show("m1", "Hello", true)
      t.show("drop", "Temp", false)
      t.show("drop", "", false)
      local p = t.pick({"a", "b"}, "  **Pick one**  ")
      local q = t.ask("Why?")
      local c = t.pick({"x"}, "  ")
      t.show("done", tostring(p) .. ":" .. tostring(q) .. ":" .. tostring(c), false)
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
        json!([
            {"name": "deploy", "desc": "ship it", "args": ["staging", "prod"], "params": 1},
            {"name": "rewrite", "desc": "", "args": [], "params": 0},
            {"name": "ui_demo", "desc": "", "args": [], "params": 0},
        ])
    );
    assert!(
        call(
            &tx,
            &out,
            901,
            "commands.run",
            json!({"name": "deploy", "argv": ["prod"]})
        )["error"]
            .is_null()
    );
    assert_eq!(
        call(
            &tx,
            &out,
            902,
            "commands.run",
            json!({"name": "deploy", "argv": ["nope"]})
        )["error"]["code"],
        -32000
    );

    assert!(call(&tx, &out, 3, "commands.run", json!({"name": "ui_demo"}))["error"].is_null());
    let r1 = event(&out, "ui_request");
    assert_eq!(r1["kind"], "picker");
    assert_eq!(r1["title"], "  **Pick one**  ");
    assert_eq!(r1["options"], json!(["a", "b"]));
    let active = &call(&tx, &out, 301, "ui.state", json!({}))["result"]["active"];
    assert_eq!(active["title"], r1["title"]);
    assert_eq!(active["options"], r1["options"]);
    assert_eq!(
        call(
            &tx,
            &out,
            302,
            "ui.reply",
            json!({"id": r1["id"], "value": "c"})
        )["error"]["code"],
        -32000
    );
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
    let result = event(&out, "ui_result");
    assert_eq!(result["value"], "b");
    assert_eq!(result["prompt"], r1["title"]);
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
    assert!(r3.get("title").is_none());
    let active = &call(&tx, &out, 303, "ui.state", json!({}))["result"]["active"];
    assert_eq!(active["id"], r3["id"]);
    assert!(active.get("title").is_none());
    assert!(call(&tx, &out, 6, "ui.cancel", json!({"id": r3["id"]}))["error"].is_null());
    let done = until(&out, |v| {
        assert_ne!(
            v["params"]["type"], "ui_clear",
            "a successful ui.cancel emits no ui_clear"
        );
        v["method"] == "event"
            && v["params"]["type"] == "ui_markdown"
            && v["params"]["name"] == "done"
    })["params"]
        .clone();
    let st = call(&tx, &out, 61, "ui.state", json!({}))["result"].clone();
    assert!(st["active"].is_null(), "{st}");
    assert_eq!(done["text"], "b:because:nil");

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
    let history = call(&tx, &out, 13, "session.history", json!({}))["result"].clone();
    assert!(history.as_array().unwrap().iter().any(|t| {
        t["role"] == "user"
            && t["context"]
                .as_str()
                .unwrap()
                .contains("<prompt name=\"review\">Check edge cases.</prompt>")
    }));

    let usage = call(&tx, &out, 130, "session.snapshot", json!({}))["result"]["usage"].clone();
    assert!(call(&tx, &out, 131, "commands.run", json!({"name": "rewrite"}))["error"].is_null());
    event(&out, "history_replaced");
    let snap = call(&tx, &out, 132, "session.snapshot", json!({}))["result"].clone();
    assert_eq!(snap["history"][0]["text"], "custom system");
    assert_eq!(snap["usage"], usage);

    let s2 = call(&tx, &out, 14, "session.new", json!({}))["result"].clone();
    assert_ne!(s2["session"]["id"], s1_id);
    let list = call(&tx, &out, 140, "session.list", json!({}))["result"].clone();
    assert_eq!(
        list.as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [s1_id.as_str()]
    );
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
    assert!(rui["active"].is_null(), "{rui}");
    assert_eq!(rui["entries"].as_array().unwrap().len(), 2);
    assert!(
        rui["markdowns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["name"] == "m1" && m["pin"] == true)
    );
    assert!(
        rui["markdowns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["name"] == "done" && m["text"] == "b:because:nil")
    );
    assert!(
        rui["markdowns"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["name"] != "drop")
    );

    call(&tx, &out, 19, "shutdown", json!({}));
    server.join().unwrap();
    common::cleanup(&dir);
}
