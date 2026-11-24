#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use support::{call, event, spawn, until};
use tuse_core::{Mode, Options, Session, Start};

fn capture_provider(replies: Vec<&'static str>) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    std::thread::spawn(move || {
        for (n, stream) in listener.incoming().flatten().enumerate() {
            let reply = replies[n.min(replies.len() - 1)];
            let seen3 = seen2.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                if let Ok(v) = serde_json::from_slice::<Value>(&body) {
                    seen3.lock().unwrap().push(v);
                }
                let mut out = stream;
                let _ = out.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                );
                let c = json!({"choices":[{"delta":{"content":reply}}]});
                let u = json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":3}});
                let _ = write!(out, "data: {c}\n\ndata: {u}\n\ndata: [DONE]\n\n");
            });
        }
    });
    (addr, seen)
}

#[test]
fn persisted_ui_history_resume_hook_ordering_ids_context_and_ui_only_quit() {
    let (addr, requests) = capture_provider(vec!["reply-1", "reply-2", "reply-3"]);
    let cfg_body = r#"
  commands = {
    ui_only = function()
      local t = require("tuse")
      t.show("only", "ephemeral", false)
      t.pick({"p1"})
      t.ask("ephemeral-q")
    end,
    step1 = function()
      local t = require("tuse")
      t.show("a", "v1", false)
      t.show("tmp", "drop-me", false)
      t.show("p", "pin-v1", true)
      local c = t.pick({"opt1", "opt2"})
      local a = t.ask("SECRET_QUESTION_PROMPT")
      local _ = t.ask("CANCELLED_PROMPT")
      t.show("tmp", "", false)
      t.show("done1", tostring(c) .. ":" .. tostring(a), false)
    end,
    step2 = function()
      local t = require("tuse")
      t.show("a", "v2-updated", false)
      t.show("p", "p-unpinned", false)
      t.show("b", "SECRET_UI_BLOCK", false)
      t.show("keep_pin", "pinned-tail", true)
      t.ask("PENDING_AT_CLOSE")
    end,
    remove_b = function()
      require("tuse").show("b", "", false)
    end,
    ask_again = function()
      local t = require("tuse")
      local v = t.ask("AFTER_RESUME_ASK")
      t.show("after_ans", tostring(v), false)
    end,
  },
"#;
    let (dir, cfg) = common::setup("ui-history", &addr, cfg_body);

    let s_only = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let only_id = s_only.id.clone();
    let (tx0, out0, srv0) = spawn(s_only);
    call(&tx0, &out0, 1, "initialize", json!({}));
    call(&tx0, &out0, 2, "commands.run", json!({"name": "ui_only"}));
    let rp = event(&out0, "ui_request");
    call(
        &tx0,
        &out0,
        3,
        "ui.reply",
        json!({"id": rp["id"], "value": "p1"}),
    );
    let rq = event(&out0, "ui_request");
    call(
        &tx0,
        &out0,
        4,
        "ui.reply",
        json!({"id": rq["id"], "value": "ans"}),
    );
    event(&out0, "ui_result");
    call(&tx0, &out0, 5, "shutdown", json!({}));
    srv0.join().unwrap();
    assert!(tuse_core::Meta::list(&dir).iter().all(|m| m.id != only_id));
    assert!(!tuse_core::tuse_home().join(&only_id).join("lock").exists());

    let s1 = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let s1_id = s1.id.clone();
    let (tx, out, srv) = spawn(s1);
    call(&tx, &out, 1, "initialize", json!({}));
    call(&tx, &out, 2, "turn.submit", json!({"text": "turn one"}));
    event(&out, "turn_end");

    call(&tx, &out, 3, "commands.run", json!({"name": "step1"}));
    let r1 = event(&out, "ui_request");
    assert_eq!(r1["id"], 1);
    call(&tx, &out, 4, "ui.reply", json!({"id": 1, "value": "opt2"}));
    event(&out, "ui_result");
    let r2 = event(&out, "ui_request");
    assert_eq!(r2["id"], 2);
    call(
        &tx,
        &out,
        5,
        "ui.reply",
        json!({"id": 2, "value": "SECRET_ANSWER"}),
    );
    event(&out, "ui_result");
    let r3 = event(&out, "ui_request");
    assert_eq!(r3["id"], 3);
    call(&tx, &out, 6, "ui.cancel", json!({"id": 3}));
    until(&out, |v| {
        v["method"] == "event"
            && v["params"]["type"] == "ui_markdown"
            && v["params"]["name"] == "done1"
    });

    call(&tx, &out, 7, "turn.submit", json!({"text": "turn two"}));
    event(&out, "turn_end");

    call(&tx, &out, 8, "commands.run", json!({"name": "step2"}));
    let pending = event(&out, "ui_request");
    assert_eq!(pending["id"], 4);
    call(&tx, &out, 9, "shutdown", json!({}));
    srv.join().unwrap();

    let s2 = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: Some(s1_id.clone()),
        start: Start::Resume,
    })
    .unwrap();
    let (tx, out, srv) = spawn(s2);
    call(&tx, &out, 1, "initialize", json!({}));

    let st = call(&tx, &out, 2, "ui.state", json!({}))["result"].clone();
    assert!(st["active"].is_null(), "{st}");
    let entries = st["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "{st}");
    assert_eq!(
        (entries[0]["id"].as_u64(), entries[0]["value"].as_str()),
        (Some(1), Some("opt2"))
    );
    assert_eq!(
        (entries[1]["id"].as_u64(), entries[1]["value"].as_str()),
        (Some(2), Some("SECRET_ANSWER"))
    );
    let mds = st["markdowns"].as_array().unwrap();
    assert!(mds.iter().all(|m| m["name"] != "tmp"), "{st}");
    assert!(
        mds.iter()
            .any(|m| m["name"] == "a" && m["text"] == "v2-updated" && m["pin"] == false)
    );
    assert!(
        mds.iter()
            .any(|m| m["name"] == "p" && m["text"] == "p-unpinned" && m["pin"] == false)
    );
    assert!(
        mds.iter()
            .any(|m| m["name"] == "keep_pin" && m["pin"] == true)
    );

    let recs = call(&tx, &out, 3, "session.records", json!({}))["result"]
        .as_array()
        .unwrap()
        .clone();
    let tags: Vec<&str> = recs
        .iter()
        .filter_map(|r| r["t"].as_str())
        .filter(|t| matches!(*t, "user" | "ui_markdown" | "ui_result"))
        .collect();
    assert_eq!(tags[0], "user");
    assert!(tags.contains(&"ui_markdown") && tags.contains(&"ui_result"));

    let trace = call(&tx, &out, 4, "session.trace", json!({}))["result"].to_string();
    assert!(!trace.contains("SECRET_UI_BLOCK") && !trace.contains("SECRET_ANSWER"));

    assert_eq!(
        call(&tx, &out, 5, "ui.reply", json!({"id": 3, "value": "stale"}))["error"]["code"],
        -32000
    );
    assert_eq!(
        call(&tx, &out, 6, "ui.reply", json!({"id": 4, "value": "stale"}))["error"]["code"],
        -32000
    );

    call(&tx, &out, 7, "commands.run", json!({"name": "ask_again"}));
    let new_req = event(&out, "ui_request");
    assert_eq!(new_req["id"], 3);
    call(&tx, &out, 8, "ui.reply", json!({"id": 3, "value": "fresh"}));
    event(&out, "ui_result");

    call(&tx, &out, 9, "commands.run", json!({"name": "remove_b"}));
    until(&out, |v| {
        v["method"] == "event"
            && v["params"]["type"] == "ui_markdown"
            && v["params"]["name"] == "b"
            && v["params"]["removed"] == true
    });

    call(&tx, &out, 10, "turn.submit", json!({"text": "turn three"}));
    event(&out, "turn_end");
    let last_req = requests.lock().unwrap().last().unwrap().to_string();
    assert!(
        !last_req.contains("SECRET_UI_BLOCK")
            && !last_req.contains("SECRET_ANSWER")
            && !last_req.contains("SECRET_QUESTION_PROMPT")
            && !last_req.contains("v2-updated"),
        "{last_req}"
    );

    call(&tx, &out, 11, "shutdown", json!({}));
    srv.join().unwrap();

    std::fs::write(
        &cfg,
        format!(
            r#"return {{
{cfg_body}
  hooks = {{
    on_resume = {{
      events = {{"start"}},
      run = function(e)
        if e.restored then
          local t = require("tuse")
          t.show("from_hook", "hook-block", false)
          t.ask("HOOK_PROMPT")
        end
      end,
    }},
  }},
}}"#
        ),
    )
    .unwrap();

    let s3 = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: Some(s1_id),
        start: Start::Resume,
    })
    .unwrap();
    let (tx, out, srv) = spawn(s3);
    call(&tx, &out, 1, "initialize", json!({}));
    let hreq = event(&out, "ui_request");
    assert_eq!(hreq["id"], 4);
    assert_eq!(hreq["text"], "HOOK_PROMPT");
    let st3 = call(&tx, &out, 2, "ui.state", json!({}))["result"].clone();
    assert_eq!(st3["active"]["id"], 4);
    assert_eq!(st3["entries"].as_array().unwrap().len(), 3);
    let mds3 = st3["markdowns"].as_array().unwrap();
    assert!(mds3.iter().all(|m| m["name"] != "b"), "{st3}");
    assert!(mds3.iter().any(|m| m["name"] == "from_hook"));
    assert!(
        mds3.iter()
            .any(|m| m["name"] == "a" && m["text"] == "v2-updated")
    );
    call(&tx, &out, 3, "shutdown", json!({}));
    srv.join().unwrap();

    common::cleanup(&dir);
}
