mod common;
use common::Reply;
use serde_json::json;
use std::time::{Duration, Instant};
use tuse_core::{Event, Options, Record, Session, UserInput};

const CODE: &str = r#"
write("a.txt", "one\ntwo\n")
edit("a.txt", "two", "2")
print(read("a.txt"))
print(grep("2", "."))
try:
    edit("a.txt", "missing", "x")
except ValueError as e:
    print("caught", e)
t = task_run("x = 5\nprint('boot')", name="bg")
while task_get(t.id).status == "running":
    sh("sleep 0.1")
task_add(t.id, "print(x + 1)")
while task_get(t.id).queue > 0 or task_get(t.id).status == "running":
    sh("sleep 0.1")
print(task_get(t.id).output)
t2 = task_run("while True:\n    sh('sleep 0.1')", name="loop")
sh("sleep 0.5")
print("stopped", task_stop(t2.id).status)
r = sh("echo first; sleep 3", timeout=1)
print("sh", r.timed_out, r.task)
"#;

#[test]
fn the_agent_edits_files_runs_tasks_and_reports_status() {
    let args = json!({"code": CODE, "title": "Testing the flow"}).to_string();
    let addr = common::provider(vec![
        Reply::Call("python", args),
        Reply::Call(
            "grep",
            json!({"pattern": "^2$", "path": "a.txt"}).to_string(),
        ),
        Reply::Text("all done"),
    ]);
    let (dir, cfg) = common::setup("flow", &addr, "");
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: tuse_core::Mode::Headless,
        resume: None,
        start: Default::default(),
    })
    .unwrap();
    let events = s.subscribe();
    s.submit(UserInput::text("go"));
    let (mut statuses, mut ended) = (vec![], false);
    let mut streamed = String::new();
    let mut starts = 0;
    let start = Instant::now();
    while !ended && start.elapsed() < Duration::from_secs(60) {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::ToolDelta { code, .. }) => streamed.push_str(&code),
            Ok(Event::ToolStart { .. }) => {
                assert_eq!(statuses[0], "Testing the flow");
                starts += 1;
            }
            Ok(Event::Status(t)) => statuses.push(t),
            Ok(Event::TurnEnd { .. }) => ended = true,
            _ => {}
        }
    }
    assert!(ended, "the turn did not end; statuses {statuses:?}");
    assert_eq!(streamed, CODE);
    assert_eq!(starts, 2);
    let st = s.stats();
    assert_eq!(
        (st.requests, st.turns, st.calls, st.totals.input),
        (3, 1, 2, 30)
    );
    let all = s.records();
    // `grep` called as a tool of its own runs as `python` code.
    let alias = all.iter().rev().find_map(|r| match r {
        Record::Tool {
            input,
            output,
            code,
            ..
        } => Some((input, output, code)),
        _ => None,
    });
    assert_eq!(
        alias,
        Some((
            &r#"grep(path="a.txt", pattern="^2$")"#.to_string(),
            &"a.txt:2: 2".to_string(),
            &Some(0)
        ))
    );
    let Some(Record::Tool {
        output,
        title,
        diffs,
        code,
        ..
    }) = all.iter().find(|r| matches!(r, Record::Tool { .. }))
    else {
        panic!("no tool record in {all:?}");
    };
    assert_eq!(*code, Some(0), "{output}");
    assert_eq!(title, "Testing the flow");
    assert!(
        output.contains("2\ttwo") || output.contains("2\t2"),
        "{output}"
    );
    assert!(output.contains("a.txt:2: 2"), "grep: {output}");
    assert!(output.contains("caught `old` was not found"), "{output}");
    assert!(
        output.contains("boot") && output.contains("--- task_add #1") && output.contains("\n6"),
        "{output}"
    );
    assert!(output.contains("stopped stopped"), "{output}");
    assert!(output.contains("sh True t3"), "adopted sh: {output}");
    assert_eq!(diffs.len(), 2, "write and edit each record a diff");
    assert!(
        diffs[1].diff.contains("-    2 | two") && diffs[1].diff.contains("+    2 | 2"),
        "{}",
        diffs[1].diff
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "one\n2\n"
    );
    // The indicator keeps the tool title until a new title arrives.
    assert_eq!(statuses, ["Testing the flow"]);
    // The process task is listed and stopped with the session.
    let tasks = s.tasks.list();
    assert_eq!(
        tasks.iter().map(|t| t.kind.as_str()).collect::<Vec<_>>(),
        ["repl", "repl", "process"]
    );
    s.close();
    assert!(s.tasks.list().iter().all(|t| !t.alive()));
    std::fs::remove_dir_all(dir).ok();
}
