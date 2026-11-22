mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Options, Record, Session, UserInput};

const CODE: &str = r#"
write("a.txt", "one\ntwo\n")
edit("a.txt", "two", "2")
print(read("a.txt"))
print(grep("2", {path = "."}))
local ok, e = pcall(edit, "a.txt", "missing", "x")
print("caught", e)
seen = 0
local t = spawn(function() for i = 1, 3 do seen = seen + 1; sleep(0.05) end print("bg done") end, "bg")
local r = sh("sleep 0.3; echo slept")
print("sh", r.ok, (r.out:gsub("\n", "")), seen)
local long = spawn(function() sh("sleep 30") end, "long")
task_stop(long)
sleep(0.1)
print("stopped", task_get(long).status)
print("big", #string.rep("x", 100000))
return string.rep("y", 10000)
"#;

#[test]
fn a_turn_runs_lua_tasks_and_records_everything() {
    let args = json!({"code": CODE, "title": "Testing the flow"}).to_string();
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
            Record::Tool {
                output,
                title,
                diffs,
                code,
                ..
            } => Some((output, title, diffs, code)),
            _ => None,
        })
        .collect();
    let (output, title, diffs, code) = tools[0];
    assert_eq!(*code, Some(0), "{}", &output[..600.min(output.len())]);
    assert_eq!(title, "Testing the flow");
    let head: String = output.chars().take(600).collect();
    assert!(output.contains("2\t2"), "{head}");
    assert!(output.contains("a.txt:2: 2"), "grep: {head}");
    assert!(output.contains("caught\t`old` was not found"), "{head}");
    assert!(
        output.contains("sh\ttrue\tslept\t3"),
        "a task runs while sh waits: {head}"
    );
    assert!(output.contains("stopped\tstopped"), "{head}");
    // Nothing is cut.
    assert!(
        output.contains("big\t100000") && output.ends_with(&"y".repeat(10000)),
        "{}",
        output.len()
    );
    assert_eq!(diffs.len(), 2, "write and edit each record a diff");
    assert!(diffs[1].diff.contains("-    2 | two") && diffs[1].diff.contains("+    2 | 2"));
    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "one\n2\n"
    );
    // A function called as a tool of its own is refused with a hint.
    assert!(
        tools[1].0.contains("the only tool is `lua`"),
        "{}",
        tools[1].0
    );
    assert_eq!(statuses, ["Testing the flow"]);
    let names: Vec<_> = s
        .tasks
        .list()
        .into_iter()
        .map(|t| (t.name, t.status))
        .collect();
    assert_eq!(
        names,
        [
            ("bg".into(), "done".into()),
            ("long".into(), "stopped".into())
        ]
    );
    // The task that finished told the agent.
    let told = |r: &Record| matches!(r, Record::Notification { text } if text.contains("Task t1 (bg) finished"));
    assert!(all.iter().any(told), "{all:?}");
    s.close();
    std::fs::remove_dir_all(dir).ok();
}
