use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tuse_core::{Event, Mode, Options, Record, Session, Start, UserInput};

struct MockHost {
    url: String,
    bodies: Arc<Mutex<Vec<Value>>>,
}

impl MockHost {
    fn start(replies: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let bodies = Arc::new(Mutex::new(vec![]));
        let log = bodies.clone();
        std::thread::spawn(move || {
            for (n, stream) in listener.incoming().flatten().enumerate() {
                let Some(resp) = replies.get(n).cloned() else {
                    break;
                };
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut buf = vec![0; len];
                if len > 0 {
                    let _ = r.read_exact(&mut buf);
                }
                if let Ok(v) = serde_json::from_slice::<Value>(&buf) {
                    log.lock().unwrap().push(v);
                }
                let mut s = stream;
                let _ = s.write_all(resp.as_bytes());
            }
        });
        Self { url, bodies }
    }

    fn bodies(&self) -> Vec<Value> {
        self.bodies.lock().unwrap().clone()
    }
}

fn json_resp(v: Value) -> String {
    let b = v.to_string();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
        b.len()
    )
}

fn wait_turn(rx: &std::sync::mpsc::Receiver<Event>) -> Vec<Event> {
    let mut out = vec![];
    loop {
        let ev = rx.recv_timeout(Duration::from_secs(15)).unwrap();
        let done = matches!(ev, Event::TurnEnd { .. });
        out.push(ev);
        if done {
            return out;
        }
    }
}

#[test]
fn multi_step_reasoning_replay_resume_switch_and_truncated_stream_safety() {
    let dir = std::env::temp_dir().join(format!("tuse-api-types-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    unsafe {
        std::env::set_var("HOME", &home);
    }

    let db_path = home.join(".tuse/tuse.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    {
        let c = rusqlite::Connection::open(&db_path).unwrap();
        c.execute_batch(
            "CREATE TABLE providers (name TEXT PRIMARY KEY, base_url TEXT NOT NULL, api_key TEXT NOT NULL);
             INSERT INTO providers VALUES ('legacy', 'http://127.0.0.1:1', 'k');",
        )
        .unwrap();
    }
    let migrated = tuse_core::providers().unwrap();
    assert_eq!(
        (migrated.len(), migrated[0].api.as_str()),
        (1, "openai-chat")
    );

    let anth = MockHost::start(vec![
        json_resp(json!({
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 100, "cache_read_input_tokens": 20, "cache_creation_input_tokens": 10, "output_tokens": 15},
            "content": [
                {"type": "thinking", "thinking": "check", "signature": "ANTH_SIG"},
                {"type": "redacted_thinking", "data": "ANTH_RED"},
                {"type": "tool_use", "id": "tu_1", "name": "lua", "input": {"code": "return 42"}}
            ]
        })),
        json_resp(json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 120, "output_tokens": 10},
            "content": [{"type": "text", "text": "anth ok"}]
        })),
        json_resp(json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 140, "output_tokens": 5},
            "content": [{"type": "text", "text": "after resume"}]
        })),
    ]);
    let anth2 = MockHost::start(vec![json_resp(json!({
        "stop_reason": "end_turn",
        "content": [{"type": "text", "text": "anth2 ok"}]
    }))]);
    let resp = MockHost::start(vec![
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"fc_bad\",\"type\":\"function_call\",\"call_id\":\"bad\",\"name\":\"lua\",\"arguments\":\"{\\\"code\\\":\\\"write('bad.txt','nope')\\\"}\"}}\n\n".into(),
        json_resp(json!({
            "status": "completed",
            "usage": {"input_tokens": 50, "output_tokens": 10},
            "output": [
                {"id": "rs_1", "type": "reasoning", "summary": [{"text": "r"}], "encrypted_content": "RESP_ENC"},
                {"id": "fc_1", "type": "function_call", "call_id": "call_r1", "name": "lua", "arguments": "{\"code\":\"return 9\"}"}
            ]
        })),
        json_resp(json!({
            "status": "completed",
            "usage": {"input_tokens": 60, "output_tokens": 8},
            "output": [{"id": "m_1", "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "resp ok"}]}]
        })),
    ]);
    let gem = MockHost::start(vec![
        json_resp(json!({
            "candidates": [{
                "content": {"parts": [
                    {"text": "gt", "thought": true, "thoughtSignature": "GEM_SIG1"},
                    {"functionCall": {"name": "lua", "args": {"code": "local ok, e = pcall(llm, 'x', 'gem/gemini-2.5-pro', 'max'); return tostring(ok) .. '|' .. e"}}, "thoughtSignature": "GEM_SIG2"}
                ]},
                "finishReason": "STOP"
            }],
            "usageMetadata": {"promptTokenCount": 40, "cachedContentTokenCount": 10, "candidatesTokenCount": 5, "thoughtsTokenCount": 7}
        })),
        json_resp(json!({
            "candidates": [{"content": {"parts": [{"text": "gem ok"}]}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 4, "thoughtsTokenCount": 0}
        })),
    ]);

    for (name, url, api) in [
        ("anth", &anth.url, "anthropic-messages"),
        ("anth2", &anth2.url, "anthropic-messages"),
        ("resp", &resp.url, "openai-responses"),
        ("gem", &gem.url, "google-gemini"),
    ] {
        tuse_core::add_provider(&tuse_config::Connect {
            name: name.into(),
            base_url: url.clone(),
            api_key: "k".into(),
            api: api.into(),
        })
        .unwrap();
    }
    tuse_core::set_state("model", "anth/claude-sonnet-4").unwrap();
    tuse_core::set_state("effort", "high").unwrap();
    let cfg = dir.join("tuse.lua");
    std::fs::write(&cfg, "return { settings = { retries = 1 } }\n").unwrap();

    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let sid = s.id.clone();
    let rx = s.subscribe();
    s.submit(UserInput::text("step 1"));
    wait_turn(&rx);
    assert_eq!(s.stats().totals.input, 250);
    assert_eq!(
        anth.bodies()[1]["messages"][1]["content"][0]["signature"],
        "ANTH_SIG"
    );
    s.close();

    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: Some(sid),
        start: Start::Resume,
    })
    .unwrap();
    let rx = s.subscribe();
    s.submit(UserInput::text("resumed"));
    wait_turn(&rx);
    assert_eq!(
        anth.bodies()[2]["messages"][1]["content"][0]["signature"],
        "ANTH_SIG"
    );

    s.select_model("anth2/claude-sonnet-4", "high", false)
        .unwrap();
    s.submit(UserInput::text("same api different provider"));
    wait_turn(&rx);
    assert!(
        !anth2.bodies()[0].to_string().contains("ANTH_SIG"),
        "different provider with same api+model must not receive old replay"
    );

    s.select_model("resp/gpt-5", "xhigh", false).unwrap();
    s.submit(UserInput::text("truncated then ok"));
    let evs = wait_turn(&rx);
    assert!(evs.iter().any(|e| matches!(e, Event::StreamReset)));
    assert!(
        !dir.join("bad.txt").exists(),
        "truncated tool call must never execute"
    );
    let rb = resp.bodies();
    assert!(!rb[0].to_string().contains("ANTH_SIG"));
    assert!(
        rb[2]["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|it| it["encrypted_content"] == "RESP_ENC")
    );

    s.select_model("gem/gemini-2.5-pro", "high", false).unwrap();
    s.submit(UserInput::text("switch to gemini"));
    wait_turn(&rx);
    let gb = gem.bodies();
    assert!(!gb[0].to_string().contains("RESP_ENC"));
    let contents = gb[1]["contents"].as_array().unwrap();
    let gem_model = &contents[contents.len() - 2];
    let gem_result = &contents[contents.len() - 1];
    assert_eq!(gem_model["parts"][0]["thoughtSignature"], "GEM_SIG1");
    assert_eq!(gem_model["parts"][1]["thoughtSignature"], "GEM_SIG2");
    assert!(
        gem_result["parts"][0]["functionResponse"]
            .get("id")
            .is_none(),
        "id-less wire functionCall must pair with id-less wire functionResponse"
    );
    assert!(s.records().iter().any(|r| matches!(
        r,
        Record::Tool { output, .. } if output.starts_with("false|")
            && output.contains("llm: provider `gem` (google-gemini) does not support effort `max`")
    )));

    s.select_model("gem/gemini-2.5-pro", "max", false).unwrap();
    s.submit(UserInput::text("bad effort"));
    let evs = wait_turn(&rx);
    assert!(evs.iter().any(|e| matches!(
        e,
        Event::Error(msg) if msg.contains("provider `gem` (google-gemini) does not support effort `max`")
    )));
    assert_eq!(gem.bodies().len(), 2);

    s.select_model("gem/gemini-2.5-pro", "default", false)
        .unwrap();
    s.submit(UserInput {
        text: "bad image".into(),
        images: vec!["data:image/png;base64,not!valid".into()],
        ..Default::default()
    });
    let evs = wait_turn(&rx);
    assert!(evs.iter().any(|e| matches!(
        e,
        Event::Error(msg) if msg.contains("does not support image")
    )));
    assert_eq!(gem.bodies().len(), 2);

    s.close();
    let _ = std::fs::remove_dir_all(&dir);
}
