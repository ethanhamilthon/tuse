mod common;
use common::Reply;
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

/// `setup` points `TUSE_HOME` at a folder per test, so the tests of this file take turns.
static ENV: Mutex<()> = Mutex::new(());

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
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
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
    // The hidden context reaches /trace but not the hooks, the title or the UI.
    s.submit(UserInput {
        context: "<prompt name=\"p\">secret</prompt>".into(),
        ..UserInput::text("hi")
    });
    let shown =
        std::iter::from_fn(|| events.recv_timeout(Duration::from_secs(10)).ok()).find_map(|e| {
            match e {
                Event::User { text, .. } => Some(text),
                _ => None,
            }
        });
    assert_eq!(shown.as_deref(), Some("hi"));
    wait_turn(&events);
    let user = s.trace().into_iter().find(|it| it.role == "user").unwrap();
    assert_eq!(user.text, "hi\n\n<prompt name=\"p\">secret</prompt>");
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

fn wait_lines(path: &std::path::Path, n: usize) -> Vec<String> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.lines().count() >= n {
            return text.lines().map(String::from).collect();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("the hook wrote fewer than {n} lines");
}

#[test]
fn agent_message_lists_only_running_tasks_and_children() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let code = "task_run(\"while True:\\n    sh('sleep 0.1')\", name='loop')";
    let addr = common::provider(vec![
        Reply::Text("one"),
        Reply::Call("python", json!({"code": code, "title": "t"}).to_string()),
        Reply::Text("two"),
        Reply::Stall("partial"),
    ]);
    let (dir, _) = common::setup("hook-running", &addr, "");
    let log = dir.join("events");
    let cfg = dir.join("main.yaml");
    std::fs::write(
        &cfg,
        format!(
            r#"model: p/m
connect: [{{name: p, base_url: 'http://{addr}', api_key: k}}]
hooks:
  record:
    events: [user_message, agent_message]
    run: |
      def names(v, key):
          return None if v is None else [x[key] for x in v]
      with open('{}', 'a') as f:
          f.write(f"{{event}} {{status}} {{names(tasks, 'name')}} {{names(children, 'id')}}\n")
"#,
            log.display()
        ),
    )
    .unwrap();
    let open = || {
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: Mode::Headless,
            resume: None,
            start: Start::Launch,
        })
        .unwrap()
    };
    let s = open();
    let events = s.subscribe();
    s.submit(UserInput::text("a"));
    wait_turn(&events);
    let lines = wait_lines(&log, 2);
    assert_eq!(lines[0], "user_message None None None");
    assert_eq!(lines[1], "agent_message finished [] []");

    // A child with an explicit parent is found before its first prompt.
    // SAFETY: the tests of this file take turns on `ENV`.
    unsafe { std::env::set_var("TUSE_PARENT", &s.id) };
    let child = open();
    unsafe { std::env::remove_var("TUSE_PARENT") };
    let found = tuse_core::descendants(&s.id, &s.config());
    assert_eq!(found.len(), 1);
    assert!(found[0].running && found[0].id == child.id);

    s.submit(UserInput::text("b"));
    wait_turn(&events);
    let lines = wait_lines(&log, 4);
    assert_eq!(
        lines[3],
        format!("agent_message finished ['loop'] ['{}']", child.id)
    );

    s.submit(UserInput::text("c"));
    wait_text(&events);
    s.cancel();
    let lines = wait_lines(&log, 6);
    assert_eq!(
        lines[5],
        format!("agent_message cancelled ['loop'] ['{}']", child.id)
    );

    child.close();
    s.close();
    std::fs::remove_dir_all(dir).ok();
}
