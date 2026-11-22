mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Options, Record, Session, UserInput};

#[test]
fn a_session_records_its_parent_and_hands_its_own_id_to_subprocesses() {
    let code = r#"print("got=" + sh("echo $TUSE_PARENT").out.strip())"#;
    let args = json!({"code": code, "title": "env"}).to_string();
    let addr = common::provider(vec![Reply::Call("python", args), Reply::Text("done")]);
    let (dir, cfg) = common::setup("parent", &addr, "");
    // SAFETY: this file holds one test, so nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("TUSE_PARENT", "outer") };
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: tuse_core::Mode::Json,
        resume: None,
        start: Default::default(),
    })
    .unwrap();
    let events = s.subscribe();
    s.submit(UserInput::text("go"));
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if let Ok(Event::TurnEnd { .. }) = events.recv_timeout(Duration::from_millis(200)) {
            break;
        }
    }
    let meta = std::fs::read_to_string(dir.join("home").join(&s.id).join("meta.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&meta).unwrap()["parent"],
        "outer"
    );
    let want = format!("got={}", s.id);
    let seen = s
        .records()
        .iter()
        .any(|r| matches!(r, Record::Tool { output, .. } if output.contains(&want)));
    assert!(seen, "{:?}", s.records());
    s.close();
    std::fs::remove_dir_all(dir).ok();
}
