mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

fn wait_turn(events: &std::sync::mpsc::Receiver<Event>) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if let Ok(Event::TurnEnd { .. }) = events.recv_timeout(Duration::from_millis(50)) {
            return;
        }
    }
    panic!("the turn did not end");
}

fn wait_text(events: &std::sync::mpsc::Receiver<Event>) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if let Ok(Event::Text(_)) = events.recv_timeout(Duration::from_millis(50)) {
            return;
        }
    }
    panic!("no text streamed");
}

#[test]
fn hooks_see_process_and_message_events_in_order() {
    let addr = common::provider(vec![
        Reply::Call("python", json!({"code": "1", "title": "t"}).to_string()),
        Reply::Text("done"),
        Reply::Stall("partial"),
    ]);
    let (dir, _) = common::setup("hooks", &addr, "");
    let log = dir.join("events");
    let cfg = dir.join("main.yaml");
    std::fs::write(
        &cfg,
        format!(
            r#"model: p/m
connect: [{{name: p, base_url: 'http://{addr}', api_key: k}}]
functions:
  label:
    description: Label the current event
    run: return event
hooks:
  record:
    events: [process_start, process_end, user_message, agent_message, turn_end]
    run: |
      if event == 'process_start':
          n = 0
      n += 1
      extra = {{'process_start': reason, 'process_end': reason, 'user_message': f'{{text}} {{is_first}}', 'agent_message': f'{{text}} {{status}}'}}[event]
      with open('{}', 'a') as f:
          f.write(f'{{label()}} {{n}} {{extra}}\n')
      if event == 'user_message' and is_first:
          set_title('hook ' + text)
"#,
            log.display()
        ),
    )
    .unwrap();
    let open = |resume, start| {
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: Mode::Headless,
            resume,
            start,
        })
        .unwrap()
    };
    let s = open(None, Start::Launch);
    assert!(
        s.config()
            .problems()
            .iter()
            .any(|p| p.contains("unknown event `turn_end`")),
        "{:?}",
        s.config().problems()
    );
    let events = s.subscribe();
    s.submit(UserInput::text("hi"));
    wait_turn(&events);
    s.reload().unwrap();
    s.submit(UserInput::text("stop me"));
    wait_text(&events);
    std::thread::sleep(Duration::from_millis(200));
    s.cancel();
    s.close_for("resume");
    let again = open(Some(s.id.clone()), Start::Resume);
    again.close();
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "process_start 1 launch
user_message 2 hi True
agent_message 3 done finished
process_end 4 reload
process_start 1 reload
user_message 2 stop me False
agent_message 3 partial cancelled
process_end 4 resume
process_start 1 resume
process_end 2 exit
"
    );
    assert_eq!(again.title(), "hook hi");
    // Hook runs are kept in the history and listed in /trace after a resume.
    let hooks: Vec<_> = again
        .trace()
        .into_iter()
        .filter(|it| it.role == "hook")
        .map(|it| (it.summary, it.text))
        .collect();
    assert!(
        hooks
            .iter()
            .any(|(summary, text)| summary == "record user_message ok"
                && text.contains("text='hi'\nis_first=True")),
        "{hooks:?}"
    );
    common::cleanup(&dir);
}
