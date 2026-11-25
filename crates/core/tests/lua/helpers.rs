#[path = "../common/mod.rs"]
mod common;
use common::Reply;
use std::time::Duration;
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

const PROBE: &str = "local d = docs('lua'); return table.concat({type(drop), keep and keep() or '-', tostring(mine), rebound(), gone(), tostring(d:find('`drop(', 1, true) ~= nil)}, ' ')";

fn config(keep: &str, drop: bool) -> String {
    let mut f = format!("keep = function() return '{keep}' end,");
    f.push_str("rebound = function() return 'config' end,");
    if drop {
        f.push_str("drop = function() return 'drop' end,");
        f.push_str("gone = function() return 'gone' end,");
    }
    format!("return {{ functions = {{ {f} }} }}")
}

fn turn(s: &std::sync::Arc<Session>, rx: &std::sync::mpsc::Receiver<Event>) -> String {
    s.submit(UserInput::text("go"));
    let mut out = String::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::ToolEnd { output, .. } => out = output,
            Event::TurnEnd { .. } => return out,
            _ => {}
        }
    }
}

#[test]
fn reload_replaces_config_helpers_and_keeps_agent_globals() {
    let set = "mine = 42; rebound = function() return 'agent' end; gone = function() return 'mine' end; return keep() .. drop()";
    let addr = common::provider(vec![
        Reply::Call("lua", serde_json::json!({ "code": set }).to_string()),
        Reply::Text("ok"),
        Reply::Call("lua", serde_json::json!({ "code": PROBE }).to_string()),
        Reply::Text("ok"),
        Reply::Call("lua", serde_json::json!({ "code": PROBE }).to_string()),
        Reply::Text("ok"),
    ]);
    let (dir, cfg) = common::setup("reload-helpers", &addr, "");
    std::fs::write(&cfg, config("v1", true)).unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let rx = s.subscribe();
    assert!(turn(&s, &rx).contains("v1drop"));

    std::fs::write(&cfg, "return { typo_key = 1 }").unwrap();
    assert!(s.reload().is_err());
    assert_eq!(turn(&s, &rx).trim(), "function v1 42 agent mine false");

    std::fs::write(&cfg, config("v2", false)).unwrap();
    s.reload().unwrap();
    assert_eq!(turn(&s, &rx).trim(), "nil v2 42 config mine false");
    s.close();
    common::cleanup(&dir);
}
