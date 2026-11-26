#[path = "../common/mod.rs"]
mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Options, Record, Session, UserInput};

static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const CODE: &str = r#"
write("a.txt", "one\ntwo\n")
edit("a.txt", "two", "2")
append("a.txt", "3[lit]\n")
write("b.txt", "drop me\n")
delete("b.txt")
print("raw", read("a.txt"))
print(read("a.txt", {lines = true}))
print(grep("3[lit]", {path = {"a.txt", "."}, literal = true}))
local ok, e = pcall(edit, "a.txt", "missing", "x")
print("caught", e)
seen = 0
local t = spawn(function() for i = 1, 3 do seen = seen + 1; sh("sleep 0.05") end print("bg done") end, "bg")
local r = sh("sleep 0.3; echo slept")
print("sh", r.ok, (r.out:gsub("\n", "")), seen)
local long = spawn(function() sh("sleep 30") end, "long")
task_stop(long)
sh("sleep 0.1")
print("stopped", task_get(long).status)
print("big", #string.rep("x", 100000))
return string.rep("y", 10000)
"#;

#[test]
fn a_turn_runs_lua_tasks_and_records_everything() {
    let _home = HOME_LOCK.lock().unwrap();
    let args = json!({"code": CODE}).to_string();
    let addr = common::provider(vec![
        Reply::Call("lua", args),
        Reply::Call("grep", json!({"pattern": "x"}).to_string()),
        Reply::Text("all done"),
    ]);
    let (dir, cfg) = common::setup("flow", &addr, "");
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
    let (mut statuses, mut ended) = (vec![], false);
    let mut streamed = String::new();
    let start = Instant::now();
    while !ended && start.elapsed() < Duration::from_secs(60) {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::ToolDelta { code, .. }) => streamed.push_str(&code),
            Ok(Event::Status(t)) => statuses.push(t),
            Ok(Event::TurnEnd { .. }) => ended = true,
            _ => {}
        }
    }
    assert!(ended, "the turn did not end; statuses {statuses:?}");
    assert_eq!(streamed, CODE);
    let all = s.records();
    let tools: Vec<_> = all
        .iter()
        .filter_map(|r| match r {
            Record::Lua {
                output,
                diffs,
                exit,
                ..
            } => Some((output, diffs, exit)),
            _ => None,
        })
        .collect();
    let (output, diffs, code) = tools[0];
    assert_eq!(*code, Some(0), "{}", &output[..600.min(output.len())]);
    let head: String = output.chars().take(600).collect();
    assert!(output.contains("raw\tone\n2\n3[lit]"), "{head}");
    assert!(output.contains("2\t2"), "{head}");
    assert!(output.contains("a.txt:3: 3[lit]"), "grep: {head}");
    assert!(output.contains("caught\t`old` was not found"), "{head}");
    assert!(
        output.contains("sh\ttrue\tslept\t3"),
        "a task runs while sh waits: {head}"
    );
    assert!(output.contains("stopped\tstopped"), "{head}");
    assert!(
        output.contains("big\t100000") && output.ends_with(&"y".repeat(10000)),
        "{}",
        output.len()
    );
    assert_eq!(
        diffs.len(),
        5,
        "write, edit, append, write, delete record diffs"
    );
    assert!(diffs[1].diff.contains("-    2 | two") && diffs[1].diff.contains("+    2 | 2"));
    assert!(diffs[2].diff.contains("+    3 | 3[lit]"));
    assert!(diffs[4].diff.contains("-    1 | drop me"));
    assert!(!dir.join("b.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "one\n2\n3[lit]\n"
    );
    assert!(
        tools[1].0.contains("the only tool is `lua`"),
        "{}",
        tools[1].0
    );
    assert_eq!(
        statuses[..9],
        [
            "writing a.txt...",
            "editing a.txt...",
            "appending a.txt...",
            "writing b.txt...",
            "deleting b.txt...",
            "reading a.txt...",
            "searching 3[lit]...",
            "editing a.txt...",
            "running sleep 0.3; echo slept..."
        ]
    );
    let names: Vec<_> = s.tasks().into_iter().map(|t| (t.name, t.status)).collect();
    assert_eq!(
        names,
        [
            ("bg".into(), "done".into()),
            ("long".into(), "stopped".into())
        ]
    );
    let told = |r: &Record| matches!(r, Record::Notification { text } if text.contains("Task t1 (bg) finished"));
    assert!(all.iter().any(told), "{all:?}");
    s.close();
    std::fs::remove_dir_all(dir).ok();
}
#[test]
fn steer_hands_off_and_esc_stops_everything() {
    let _home = HOME_LOCK.lock().unwrap();
    for steer in [true, false] {
        let code = r#"
return coroutine.wrap(function()
local saved = 41
print('before')
write('foreground', 'yes')
local r = sh('echo run >> starts; while [ ! -f go ]; do sleep 0.02; done; printf ready', {timeout = 12})
assert(r.ok, tostring(r))
write('background', sh('printf next').out)
return r.out .. ':' .. (saved + 1)
end)()
"#;
        let addr = common::provider(vec![
            Reply::Call("lua", json!({"code": code}).to_string()),
            Reply::Text("handled"),
            Reply::Text("again"),
        ]);
        let (dir, cfg) = common::setup(if steer { "steer-handoff" } else { "esc-stop" }, &addr, "");
        let s = Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg),
            mode: tuse_core::Mode::Json,
            resume: None,
            start: Default::default(),
        })
        .unwrap();
        let rx = s.subscribe();
        s.submit(UserInput::text("start"));
        let until = Instant::now() + Duration::from_secs(5);
        while !dir.join("starts").exists() {
            assert!(Instant::now() < until, "shell did not start");
            std::thread::sleep(Duration::from_millis(5));
        }
        if steer {
            assert!(s.submit(UserInput::text("steer now")).1);
        } else {
            s.cancel();
        }
        loop {
            match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::ToolEnd { output, .. } if steer => {
                    assert_eq!(output, "[continued in background: t1]");
                }
                Event::TurnEnd { cancelled, .. } => {
                    assert_eq!(cancelled, !steer);
                    break;
                }
                _ => {}
            }
        }
        if !steer {
            assert!(s.tasks().is_empty(), "{:?}", s.tasks());
            let out = s.with_records(|r| {
                r.iter()
                    .find_map(|r| match r {
                        Record::Lua { output, .. } => Some(output.clone()),
                        _ => None,
                    })
                    .unwrap()
            });
            assert_eq!(out, "[cancelled]");
            s.close();
            common::cleanup(&dir);
            continue;
        }
        assert_eq!(s.tasks()[0].name, "lua-1");
        std::fs::write(dir.join("go"), "").unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while s.tasks()[0].status == "running" {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
        loop {
            if let Event::Notification(text) = rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                assert!(
                    text.starts_with("<task>Task t1 (lua-1) finished. Last line: ready:42."),
                    "{text}"
                );
                break;
            }
        }
        loop {
            if let Event::TurnEnd { .. } = rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                break;
            }
        }
        assert_eq!(
            std::fs::read_to_string(dir.join("background")).unwrap(),
            "next"
        );
        s.close();
        common::cleanup(&dir);
    }
}

#[test]
fn tool_history_write_waits_for_the_tool_result() {
    let _home = HOME_LOCK.lock().unwrap();
    let args = json!({"code": "set_session{history={{role='system', text='short'}}}; assert(session().history[1].text ~= 'short'); return 'tool finished'"}).to_string();
    let addr = common::provider(vec![Reply::Call("lua", args), Reply::Text("done")]);
    let (dir, cfg) = common::setup("pending-history", &addr, "");
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: tuse_core::Mode::Json,
        resume: None,
        start: Default::default(),
    })
    .unwrap();
    let rx = session.subscribe();
    session.submit(UserInput::text("go"));
    let mut result = false;
    let mut replaced = false;
    loop {
        match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::ToolEnd { output, .. } => {
                assert_eq!(output, "tool finished");
                result = true;
            }
            Event::HistoryReplaced => {
                assert!(result);
                replaced = true;
            }
            Event::TurnEnd { .. } => break,
            Event::Error(e) => panic!("{e}"),
            _ => {}
        }
    }
    assert!(replaced);
    let snap = session.snapshot(true);
    assert_eq!(snap.history.unwrap()[0].fields["text"], "short");
    assert_eq!(snap.usage.unwrap().input, 20);
    assert_eq!(snap.started, Some(true));
    session.close();
    common::cleanup(&dir);
}
