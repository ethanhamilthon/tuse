#[path = "../../core/tests/common/mod.rs"]
mod common;
mod support;

use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use support::{call, event, spawn};
use tuse_core::{Event, Mode, Options, Record, Session, Start, UserInput};

fn wait_turn(rx: &std::sync::mpsc::Receiver<Event>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if matches!(
            rx.recv_timeout(Duration::from_millis(50)),
            Ok(Event::TurnEnd { .. })
        ) {
            return;
        }
    }
    panic!("turn did not end");
}

#[test]
fn cross_frontend_resume_preserves_history_model_and_persisted_ui() {
    let addr = common::provider(vec![
        Reply::Text("rpc reply"),
        Reply::Text("tui continued"),
        Reply::Text("tui reply"),
        Reply::Text("rpc continued"),
    ]);
    let extra = r#"
  commands = {
    ui = { run = function()
      local t = require("tuse")
      t.show("pin", "Stay?", true)
      t.ask("Pending?")
    end },
  },
"#;
    let (dir, cfg) = common::setup("cross-resume", &addr, extra);
    let open = |mode, resume| {
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode,
            resume,
            start: Start::Launch,
        })
    };

    let rpc_s = open(Mode::Json, None).unwrap();
    let id1 = rpc_s.id.clone();
    let (tx, out, server) = spawn(rpc_s);
    call(&tx, &out, 1, "initialize", json!({}));
    call(
        &tx,
        &out,
        2,
        "model.set",
        json!({"model": "p/alt", "effort": "high"}),
    );
    call(
        &tx,
        &out,
        3,
        "turn.submit",
        json!({"text": "hello from rpc"}),
    );
    event(&out, "turn_end");
    call(&tx, &out, 4, "commands.run", json!({"name": "ui"}));
    event(&out, "ui_request");
    call(&tx, &out, 5, "shutdown", json!({}));
    server.join().unwrap();

    let tui_s = open(Mode::Tui, Some(id1.clone())).unwrap();
    assert!(
        open(Mode::Json, Some(id1)).is_err(),
        "same lock must exclude JSON-RPC while TUI holds it"
    );
    assert_eq!(tui_s.model(), ("p/alt".into(), "high".into()));
    assert!(
        tui_s
            .records()
            .iter()
            .any(|r| matches!(r, Record::Agent { text, .. } if text == "rpc reply"))
    );
    let snap = tui_s.ui_snapshot();
    assert_eq!(snap.markdowns.len(), 1);
    assert_eq!(
        (
            snap.markdowns[0].name.as_str(),
            snap.markdowns[0].text.as_str(),
            snap.markdowns[0].pin
        ),
        ("pin", "Stay?", true)
    );
    assert!(snap.active.is_none() && snap.entries.is_empty());
    let ev = tui_s.subscribe();
    tui_s.submit(UserInput::text("continue in tui"));
    wait_turn(&ev);
    assert!(
        tui_s
            .records()
            .iter()
            .any(|r| matches!(r, Record::Agent { text, .. } if text == "tui continued"))
    );
    tui_s.close();

    let tui_s2 = open(Mode::Tui, None).unwrap();
    let id2 = tui_s2.id.clone();
    tui_s2.select_model("p/m", "low", false).unwrap();
    let ev2 = tui_s2.subscribe();
    tui_s2.submit(UserInput::text("hello from tui"));
    wait_turn(&ev2);
    tui_s2.run_command("ui", &[]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while tui_s2.ui_snapshot().active.is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    tui_s2.close();

    let rpc_s2 = open(Mode::Json, Some(id2.clone())).unwrap();
    assert!(
        open(Mode::Tui, Some(id2.clone())).is_err(),
        "same lock must exclude TUI while JSON-RPC holds it"
    );
    let (tx, out, server) = spawn(rpc_s2);
    let init = call(&tx, &out, 1, "initialize", json!({}))["result"].clone();
    assert_eq!(
        (
            init["session"]["id"].clone(),
            init["session"]["model"].clone(),
            init["session"]["effort"].clone()
        ),
        (json!(id2), json!("p/m"), json!("low"))
    );
    let st = call(&tx, &out, 2, "ui.state", json!({}))["result"].clone();
    assert_eq!(st["markdowns"].as_array().unwrap().len(), 1);
    assert_eq!(st["markdowns"][0]["name"], "pin");
    assert_eq!(st["markdowns"][0]["pin"], true);
    assert!(st["active"].is_null() && st["entries"].as_array().unwrap().is_empty());
    let recs = call(&tx, &out, 3, "session.history", json!({}))["result"].clone();
    assert!(
        recs.as_array()
            .unwrap()
            .iter()
            .any(|r| r["role"] == "user" && r["text"] == "hello from tui")
    );
    assert!(
        call(
            &tx,
            &out,
            4,
            "turn.submit",
            json!({"text": "continue in rpc"})
        )["error"]
            .is_null()
    );
    assert_eq!(event(&out, "assistant")["text"], "rpc continued");
    event(&out, "turn_end");
    call(&tx, &out, 5, "shutdown", json!({}));
    server.join().unwrap();

    common::cleanup(&dir);
}
