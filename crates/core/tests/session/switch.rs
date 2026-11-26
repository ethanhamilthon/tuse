#[path = "../common/mod.rs"]
mod common;
use common::Reply;
use std::time::Duration;
use tuse_core::{Event, Mode, Options, Record, Session, Start, Switch, UserInput};

fn open(name: &str, replies: Vec<Reply>) -> (std::sync::Arc<Session>, std::path::PathBuf) {
    let addr = common::provider(replies);
    let (dir, cfg) = common::setup(name, &addr, "");
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    (s, dir)
}

fn call(code: &str) -> Reply {
    Reply::Call("lua", serde_json::json!({ "code": code }).to_string())
}

#[test]
fn switching_from_lua_waits_for_tool_calls_and_validates() {
    fork_waits_for_the_result_and_stops_the_turn();
    bad_arguments_raise_and_a_second_switch_is_refused();
}

fn fork_waits_for_the_result_and_stops_the_turn() {
    let (s, dir) = open(
        "switch-fork",
        vec![
            call("fork(#session().history); return 'asked'"),
            Reply::Text("late"),
        ],
    );
    let rx = s.subscribe();
    s.submit(UserInput::text("go"));
    let mut seen = vec![];
    while let Ok(e) = rx.recv_timeout(Duration::from_secs(10)) {
        match e {
            Event::ToolEnd { .. } => seen.push("result"),
            Event::Switch(Switch::Fork(index)) => {
                assert!(matches!(s.records()[index], Record::Assistant { .. }));
                seen.push("switch");
            }
            Event::TurnEnd { .. } => break,
            _ => {}
        }
    }
    assert_eq!(seen, ["result", "switch"]);
    let records = s.records();
    assert!(records.iter().any(|r| matches!(r, Record::Tool { .. })));
    let agents = records
        .iter()
        .filter(|r| matches!(r, Record::Assistant { .. }))
        .count();
    assert_eq!(agents, 1);
    s.close();
    common::cleanup(&dir);
}

fn bad_arguments_raise_and_a_second_switch_is_refused() {
    let code = "local out = {}
for _, f in ipairs({ function() fork(1) end, function() fork(99) end, function() resume('nope') end }) do
  local _, e = pcall(f)
  out[#out + 1] = tostring(e)
end
fork(#session().history)
local _, e = pcall(resume, 'other')
out[#out + 1] = tostring(e)
return table.concat(out, '|')";
    let (s, dir) = open("switch-bad", vec![call(code), Reply::Text("ok")]);
    let rx = s.subscribe();
    s.submit(UserInput::text("go"));
    let mut output = String::new();
    while let Ok(e) = rx.recv_timeout(Duration::from_secs(10)) {
        match e {
            Event::ToolEnd { output: o, .. } => output = o,
            Event::TurnEnd { .. } => break,
            _ => {}
        }
    }
    for want in [
        "record 1 is not a user or agent message",
        "no record 99",
        "no session nope",
        "a session switch is already pending",
    ] {
        assert!(
            output.contains(want),
            "{want}: {output:?} {:?}",
            s.records()
                .iter()
                .filter(|r| !matches!(r, Record::System { .. }))
                .collect::<Vec<_>>()
        );
    }
    s.close();
    common::cleanup(&dir);
}
