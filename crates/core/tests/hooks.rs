mod common;
#[path = "hooks/running.rs"]
mod running;
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
        Reply::Call("lua", json!({"code": "1"}).to_string()),
        Reply::Text("done"),
        Reply::Stall("partial"),
    ]);
    let (dir, _) = common::setup("hooks", &addr, "");
    let log = dir.join("events");
    let tui_log = dir.join("tui");
    let hl_log = dir.join("hl");
    let cfg = dir.join("main.lua");
    std::fs::write(
        &cfg,
        format!(
            r#"local n = 0
local function append(path, line)
  local f = assert(io.open(path, "a"))
  f:write(line, "\n")
  f:close()
end
return {{
  model = "p/m",
  functions = {{
    label = {{ description = "Label the current event", run = function(e) return e.event end }},
  }},
  hooks = {{
    record = {{
      events = {{"process_start", "process_end", "user_message", "agent_message", "turn_end"}},
      run = function(e)
        if e.event == "process_start" then n = 0 end
        n = n + 1
        local extra = e.reason
        if e.event == "user_message" then extra = e.text .. " " .. tostring(e.is_first) end
        if e.event == "agent_message" then extra = e.text .. " " .. e.status end
        append("{}", label(e) .. " " .. n .. " " .. extra)
        if e.event == "user_message" and e.is_first then set_title("hook " .. e.text) end
      end,
    }},
    tui_only = TUSE_MODE == "tui" and {{
      events = {{"process_start", "process_end", "user_message"}},
      run = function(e) append("{}", e.event) end,
    }} or nil,
    hl_only = TUSE_MODE == "json" and {{
      events = {{"process_start", "process_end"}},
      run = function(e) append("{}", e.event .. " " .. e.reason) end,
    }} or nil,
  }},
}}
"#,
            log.display(),
            tui_log.display(),
            hl_log.display()
        ),
    )
    .unwrap();
    let open = |resume, start| {
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: Mode::Json,
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
user_message 2 hi true
agent_message 3 done finished
process_end 4 reload
process_start 1 reload
user_message 2 stop me false
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
                && text.contains("is_first=true\ntext=\"hi\"")),
        "{hooks:?}"
    );
    assert!(!tui_log.exists(), "tui_only hook ran in json");
    assert!(!hooks.iter().any(|(s, _)| s.starts_with("tui_only")));
    assert_eq!(
        std::fs::read_to_string(&hl_log).unwrap(),
        "process_start launch\nprocess_end reload\nprocess_start reload\nprocess_end resume\nprocess_start resume\nprocess_end exit\n"
    );
    let tui = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Tui,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    tui.close();
    assert_eq!(
        std::fs::read_to_string(&tui_log).unwrap(),
        "process_start\nprocess_end\n"
    );
    assert_eq!(
        std::fs::read_to_string(&hl_log).unwrap(),
        "process_start launch\nprocess_end reload\nprocess_start reload\nprocess_end resume\nprocess_start resume\nprocess_end exit\n"
    );
    common::cleanup(&dir);
}
