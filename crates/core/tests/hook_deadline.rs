mod common;
use std::time::Duration;
use tuse_core::{Event, Mode, Options, Session, Start};

#[test]
fn hook_questions_obey_timeout_and_cancel_even_without_an_agent_turn() {
    for (timeout, cancel) in [(1, false), (60, true)] {
        let extra = format!(
            "settings: {{hook_timeout: {timeout}}}\nhooks:\n  wait:\n    events: [process_start]\n    run: ask('Ready?')\n"
        );
        let (dir, config) = common::setup("hook-deadline", "localhost", &extra);
        let session = Session::open(Options {
            cwd: dir.clone(),
            config: Some(config),
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
        })
        .unwrap();
        let events = session.subscribe();
        let next = || events.recv_timeout(Duration::from_secs(5)).unwrap();
        let id = loop {
            if let Event::Ask { id, .. } = next() {
                break id;
            }
        };
        if cancel {
            session.cancel();
        }
        loop {
            if let Event::AskDone { id: done } = next() {
                assert_eq!(done, id);
                break;
            }
        }
        assert!(!session.has_ask(&id));
        session.close();
        common::cleanup(&dir);
    }
}
