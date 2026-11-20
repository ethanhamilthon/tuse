use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};
use tuse_core::{Event, Options, Record, Session, UserInput};

/// Answers every request with SSE: a `python` call for the first one, plain text after.
fn serve(listener: TcpListener, code: String) {
    std::thread::spawn(move || {
        for (n, stream) in listener.incoming().flatten().enumerate() {
            let code = code.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let delta = if n == 0 {
                    let args = json!({"code": code, "title": "Testing the flow"}).to_string();
                    json!({"tool_calls": [{"index": 0, "id": "c1", "function": {"name": "python", "arguments": args}}]})
                } else {
                    json!({"content": "all done"})
                };
                let chunks = [
                    json!({"choices": [{"delta": delta}]}),
                    json!({"choices": [], "usage": {"prompt_tokens": 10, "completion_tokens": 3}}),
                ];
                let mut out = stream;
                let _ = out.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
                for c in chunks {
                    let _ = write!(out, "data: {c}\n\n");
                }
                let _ = out.write_all(b"data: [DONE]\n\n");
            });
        }
    });
}

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
    let dir = std::env::temp_dir().join(format!("tuse-flow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: this file holds one test, so nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("TUSE_HOME", dir.join("home")) };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    serve(listener, CODE.to_string());
    let cfg = dir.join("tuse.yaml");
    std::fs::write(
        &cfg,
        format!("model: p/m\nconnect: [{{name: p, base_url: 'http://{addr}', api_key: k}}]\n"),
    )
    .unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        headless: true,
        resume: None,
    })
    .unwrap();
    let events = s.subscribe();
    s.submit(UserInput::text("go"));
    let (mut statuses, mut ended) = (vec![], false);
    let start = Instant::now();
    while !ended && start.elapsed() < Duration::from_secs(60) {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Status(t)) => statuses.push(t),
            Ok(Event::TurnEnd { .. }) => ended = true,
            _ => {}
        }
    }
    assert!(ended, "the turn did not end; statuses {statuses:?}");
    let all = s.records();
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
    // The indicator only shows the tool title and clears when the tool finishes.
    assert_eq!(statuses, ["Testing the flow", ""]);
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
