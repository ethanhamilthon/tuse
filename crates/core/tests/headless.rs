mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Record, Session, UserInput};

#[test]
fn headless_sessions_explain_what_is_unavailable() {
    let code = "try:\n    ask('ok?')\nexcept RuntimeError as e:\n    print(e)\ntry:\n    draw_ui('x')\nexcept RuntimeError as e:\n    print(e)\n";
    let addr = common::provider(vec![
        Reply::Call("python", json!({"code": code, "title": "t"}).to_string()),
        Reply::Text("done"),
    ]);
    let (dir, cfg) = common::setup("headless", &addr, "");
    let open = |mode| {
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode,
            resume: None,
        })
        .unwrap()
    };
    let s = open(Mode::Headless);
    assert!(s.trace()[0].text.contains("Mode: headless"));
    let events = s.subscribe();
    s.submit(UserInput::text("go"));
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if let Ok(Event::TurnEnd { .. }) = events.recv_timeout(Duration::from_millis(100)) {
            break;
        }
    }
    let output = s.records().into_iter().find_map(|r| match r {
        Record::Tool { output, .. } => Some(output),
        _ => None,
    });
    let output = output.expect("no tool call");
    assert!(
        output.contains("ask is unavailable in headless mode (tuse -p)"),
        "{output}"
    );
    assert!(
        output.contains("draw_ui is unavailable in headless mode"),
        "{output}"
    );
    s.close();
    let tui = open(Mode::Tui);
    assert!(!tui.trace()[0].text.contains("Mode: headless"));
    tui.close();
    common::cleanup(&dir);
}
