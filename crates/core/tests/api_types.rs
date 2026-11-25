use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tuse_core::{Event, Mode, Options, Record, Session, Start, UserInput};

fn mock(replies: Vec<String>) -> (String, Arc<Mutex<Vec<Value>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let bodies = Arc::new(Mutex::new(vec![]));
    let log = bodies.clone();
    std::thread::spawn(move || {
        for (n, mut s) in l.incoming().flatten().enumerate() {
            let Some(resp) = replies.get(n).cloned() else {
                break;
            };
            let (mut r, mut len) = (BufReader::new(s.try_clone().unwrap()), 0usize);
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
            let _ = s.write_all(resp.as_bytes());
        }
    });
    (url, bodies)
}

fn j(b: &str) -> String {
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
fn replay_resume_provider_isolation_and_truncation() {
    let dir = std::env::temp_dir().join(format!("tuse-api-types-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = dir.join("home");
    std::fs::create_dir_all(home.join(".tuse")).unwrap();
    unsafe { std::env::set_var("HOME", &home) };

    let (au, ab) = mock(vec![
        j(
            r#"{"stop_reason":"tool_use","usage":{"input_tokens":100,"cache_read_input_tokens":20,"cache_creation_input_tokens":10,"output_tokens":15},"content":[{"type":"thinking","thinking":"c","signature":"SIG1"},{"type":"tool_use","id":"tu_1","name":"lua","input":{"code":"local ok, e = pcall(llm, 'x', 'gem/gemini-2.5-pro', 'max'); return tostring(ok) .. '|' .. e"}}]}"#,
        ),
        j(
            r#"{"stop_reason":"end_turn","usage":{"input_tokens":120,"output_tokens":10},"content":[{"type":"text","text":"ok"}]}"#,
        ),
        j(r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"resumed"}]}"#),
    ]);
    let (a2u, a2b) = mock(vec![j(
        r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"a2"}]}"#,
    )]);
    let (ru, rb) = mock(vec![
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"fc_bad\",\"type\":\"function_call\",\"call_id\":\"bad\",\"name\":\"lua\",\"arguments\":\"{\\\"code\\\":\\\"write('bad.txt','nope')\\\"}\"}}\n\n".into(),
        j(r#"{"status":"completed","output":[{"id":"rs_1","type":"reasoning","summary":[{"text":"r"}],"encrypted_content":"ENC1"},{"id":"m_1","type":"message","role":"assistant","content":[{"type":"output_text","text":"resp ok"}]}]}"#),
    ]);
    for (n, u, a) in [
        ("anth", &au, "anthropic-messages"),
        ("anth2", &a2u, "anthropic-messages"),
        ("resp", &ru, "openai-responses"),
        ("gem", &ru, "google-gemini"),
    ] {
        tuse_core::Providers::add(&tuse_config::Connect {
            name: n.into(),
            base_url: u.clone(),
            api_key: "k".into(),
            api: a.into(),
        })
        .unwrap();
    }
    tuse_core::Prefs::set("model", "anth/claude-sonnet-4").unwrap();
    tuse_core::Prefs::set("effort", "high").unwrap();
    let cfg = dir.join("tuse.lua");
    std::fs::write(&cfg, "return {}\n").unwrap();

    let open = |resume: Option<String>| {
        let start = if resume.is_some() {
            Start::Resume
        } else {
            Start::Launch
        };
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: Mode::Json,
            resume,
            start,
        })
        .unwrap()
    };
    let s = open(None);
    let (sid, rx) = (s.id.clone(), s.subscribe());
    s.submit(UserInput::text("s1"));
    wait_turn(&rx);
    assert!(s.stats().totals.input == 250 && ab.lock().unwrap()[1].to_string().contains("SIG1"));
    assert!(s.records().iter().any(|r| matches!(r, Record::Tool { output, .. } if output.starts_with("false|") && output.contains("does not support effort `max`"))));
    s.close();

    let s = open(Some(sid));
    let rx = s.subscribe();
    s.submit(UserInput::text("s2"));
    wait_turn(&rx);
    assert!(ab.lock().unwrap()[2].to_string().contains("SIG1"));

    std::fs::write(
        dir.join("tuse.lua"),
        r#"return { commands = { rewrite = function()
      local t = require('tuse')
      local h = t.session().history
      t.set_session{ history = h }
      for _, r in ipairs(h) do
        if r.role == 'agent' and r.replay then r.text = 'edited'; break end
      end
      t.set_session{ history = h }
    end } }"#,
    )
    .unwrap();
    s.reload().unwrap();
    s.run_command("rewrite", &[]).unwrap();
    loop {
        match rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
            Event::HistoryReplaced => break,
            Event::Error(e) => panic!("{e}"),
            _ => {}
        }
    }
    assert!(s.history().iter().any(|r| r.role == "agent"
        && r.fields["text"] == "edited"
        && !r.fields.contains_key("replay")));
    s.select_model("anth2/claude-sonnet-4", "high", false)
        .unwrap();
    s.submit(UserInput::text("s3"));
    wait_turn(&rx);
    assert!(!a2b.lock().unwrap()[0].to_string().contains("SIG1"));

    s.select_model("resp/gpt-5", "xhigh", false).unwrap();
    s.submit(UserInput::text("s4"));
    let evs = wait_turn(&rx);
    assert!(
        evs.iter().any(|e| matches!(e, Event::StreamReset))
            && !dir.join("bad.txt").exists()
            && !rb.lock().unwrap()[0].to_string().contains("SIG1")
    );

    s.select_model("gem/gemini-2.5-pro", "max", false).unwrap();
    s.submit(UserInput::text("bad effort"));
    assert!(
        wait_turn(&rx)
            .iter()
            .any(|e| matches!(e, Event::Error(m) if m.contains("does not support effort `max`")))
    );
    s.close();
    let _ = std::fs::remove_dir_all(&dir);
}
