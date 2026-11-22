mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Options, Record, Session, UserInput};

#[test]
fn the_marker_reports_the_real_number_of_cut_bytes() {
    let code = r#"print("y"*3000 + "\nMIDDLE\n" + "z"*3000)"#;
    let addr = common::provider(vec![
        Reply::Call("python", json!({"code": code}).to_string()),
        Reply::Text("done"),
    ]);
    let (dir, cfg) = common::setup("trunc", &addr, "");
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
    let mut ended = false;
    while !ended && start.elapsed() < Duration::from_secs(60) {
        if let Ok(Event::TurnEnd { .. }) = events.recv_timeout(Duration::from_millis(200)) {
            ended = true;
        }
    }
    assert!(ended);
    let out = s.records().into_iter().find_map(|r| match r {
        Record::Tool { output, .. } => Some(output),
        _ => None,
    });
    let out = out.expect("tool record");
    assert!(out.len() <= 4096, "{}", out.len());
    let n: usize = out
        .split("[... ")
        .nth(1)
        .and_then(|r| r.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no marker: {out}"));
    let marker = format!("\n[... {n} bytes cut ...]\n");
    assert_eq!(n, 6008 - (out.len() - marker.len()));
    s.close();
    std::fs::remove_dir_all(dir).ok();
}
