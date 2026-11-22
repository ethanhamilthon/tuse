mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, UserInput};

#[test]
fn headless_sessions_say_so_in_the_prompt() {
    let code = "print('ok')";
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
            start: Default::default(),
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
    s.close();
    let tui = open(Mode::Tui);
    assert!(!tui.trace()[0].text.contains("Mode: headless"));
    tui.close();
    common::cleanup(&dir);
}
