mod common;
#[path = "hooks/running.rs"]
mod running;
use common::Reply;
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

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
fn hooks_emit_event_only_payloads_and_all_lifecycle_events() {
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
            r#"local t = require("tuse")
local n = 0
local function append(path, line)
  local f = assert(io.open(path, "a"))
  f:write(line, "\n")
  f:close()
end
return {{
  functions = {{
    label = {{ description = "Label the current event", run = function(e) return e.event end }},
  }},
  hooks = {{
    record = {{
      events = {{"start", "close", "user_message", "agent_message", "model_change", "name_change"}},
      run = function(e)
        -- Event-only payloads: session metadata lives in t.session(), not on `e`.
        assert(e.session == nil and e.cwd == nil and e.config == nil and e.hook == nil)
        local s = t.session()
        assert(s.id ~= nil, "session must be readable during " .. e.event)
        if e.event == "start" then n = 0 end
        n = n + 1
        local extra = ""
        if e.event == "start" then
          extra = e.reason .. " " .. tostring(e.is_new) .. " " .. tostring(e.restored)
        elseif e.event == "close" then
          extra = e.reason .. " id=" .. (s.id ~= nil and "ok" or "nil")
        elseif e.event == "user_message" then
          extra = e.text .. " " .. tostring(e.is_first)
        elseif e.event == "agent_message" then
          local tc = e.tool_calls and #e.tool_calls or 0
          extra = e.text .. " " .. e.status .. " tc=" .. tc
        elseif e.event == "model_change" then
          assert(s.model == e.new.model and s.effort == e.new.effort)
          extra = tostring(e.previous.model) .. "/" .. e.previous.effort .. "->" .. tostring(e.new.model) .. "/" .. e.new.effort
        elseif e.event == "name_change" then
          assert(s.name == e.new)
          extra = tostring(e.previous) .. "->" .. e.new
        end
        append("{}", label(e) .. " " .. n .. " " .. extra)
      end,
    }},
    tui_only = t.session().mode == "tui" and {{
      events = {{"start", "close", "user_message"}},
      run = function(e) append("{}", e.event) end,
    }} or nil,
    hl_only = t.session().mode == "json" and {{
      events = {{"start", "close"}},
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
    let events = s.subscribe();
    s.select_model("p/alt", "high", false).unwrap();
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
    s.set_name("hook hi").unwrap();
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
        "start 1 launch true false
model_change 2 p/m/default->p/alt/high
name_change 3 nil->hi
user_message 4 hi true
agent_message 5  working tc=1
agent_message 6 done finished tc=0
name_change 7 hi->hook hi
close 8 reload id=ok
start 1 reload false false
user_message 2 stop me false
agent_message 3 partial cancelled tc=0
close 4 resume id=ok
start 1 resume false true
close 2 exit id=ok
"
    );
    assert_eq!(again.title(), "hook hi");
    assert!(!tui_log.exists(), "tui_only hook ran in json");
    assert_eq!(
        std::fs::read_to_string(&hl_log).unwrap(),
        "start launch\nclose reload\nstart reload\nclose resume\nstart resume\nclose exit\n"
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
    assert_eq!(std::fs::read_to_string(&tui_log).unwrap(), "start\nclose\n");
    common::cleanup(&dir);
}
