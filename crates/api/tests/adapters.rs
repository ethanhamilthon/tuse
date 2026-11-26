use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tuse_api::{
    ApiType, Client, Delta, Error, Message, Replay, Reply, Request, ToolCall, ToolDef, ToolImage,
    Usage,
};

type Seen = Arc<Mutex<Vec<(String, HashMap<String, String>, String)>>>;

fn serve(replies: Vec<String>) -> (String, Seen) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let seen: Seen = Arc::new(Mutex::new(vec![]));
    let log = seen.clone();
    std::thread::spawn(move || {
        for (n, mut out) in l.incoming().flatten().enumerate() {
            let Some(resp) = replies.get(n).cloned() else {
                break;
            };
            let log = log.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(out.try_clone().unwrap());
                let mut first = String::new();
                if r.read_line(&mut first).unwrap_or(0) == 0 {
                    return;
                }
                let path = first.split_whitespace().nth(1).unwrap_or("").into();
                let (mut hdrs, mut len) = (HashMap::new(), 0usize);
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = line.split_once(':') {
                        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
                        if k == "content-length" {
                            len = v.parse().unwrap_or(0);
                        }
                        hdrs.insert(k, v);
                    }
                }
                let mut buf = vec![0; len];
                if len > 0 {
                    let _ = r.read_exact(&mut buf);
                }
                log.lock()
                    .unwrap()
                    .push((path, hdrs, String::from_utf8_lossy(&buf).into()));
                if let Some(frame) = resp.strip_prefix("STALL:") {
                    let _ = out.write_all(frame.as_bytes());
                    let _ = out.flush();
                    while out.write_all(b": ping\n\n").is_ok() {
                        std::thread::sleep(std::time::Duration::from_millis(30));
                    }
                } else {
                    let _ = out.write_all(resp.as_bytes());
                }
            });
        }
    });
    (url, seen)
}

fn j(b: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
        b.len()
    )
}
fn sse(fs: &[&str]) -> String {
    let mut s = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
        .to_string();
    for f in fs {
        s.push_str("data: ");
        s.push_str(f);
        s.push_str("\n\n");
    }
    s
}
fn pv(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn run(c: &Client, r: &Request) -> Result<Reply, Error> {
    c.complete(r, 0, &AtomicBool::new(false), &mut |_| {}, &mut |_| {})
}
fn ureq(p: &str, m: &str, ef: Option<&str>) -> Request {
    Request {
        provider: p.into(),
        model: m.into(),
        system: "".into(),
        messages: vec![Message::User {
            text: "hi".into(),
            images: vec![],
        }],
        tools: vec![],
        effort: ef.map(Into::into),
    }
}
fn rich(p: &str, m: &str, ef: Option<&str>, replay: Option<Replay>) -> Request {
    let tc = ToolCall {
        id: "tc_1".into(),
        name: "lua".into(),
        arguments: r#"{"code":"return 1"}"#.into(),
    };
    let im = ToolImage {
        label: "i.png".into(),
        urls: vec!["data:image/jpeg;base64,QkJC".into()],
    };
    Request {
        provider: p.into(),
        model: m.into(),
        system: "Be concise.".into(),
        effort: ef.map(Into::into),
        tools: vec![ToolDef {
            name: "lua".into(),
            description: "Run".into(),
            parameters: serde_json::json!({"type":"object"}),
        }],
        messages: vec![
            Message::User {
                text: "look".into(),
                images: vec!["data:image/png;base64,QUFB".into()],
            },
            Message::Assistant {
                text: "check".into(),
                tool_calls: vec![tc],
                replay,
            },
            Message::ToolResult {
                id: "tc_1".into(),
                name: "lua".into(),
                output: "1".into(),
                images: Some(im),
            },
        ],
    }
}

#[test]
fn adapters_request_stream_tools_usage_replay_and_models() {
    let (u, log) = serve(vec![
        sse(&[
            r#"{"choices":[{"delta":{"reasoning_content":"a","reasoning":"b"}}]}"#,
            r#"{"choices":[{"delta":{"content":"hi"}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"lua","arguments":"{\"co"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"de\":\"p()\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":40,"cache_write_tokens":10}}}"#,
            "[DONE]",
        ]),
        j(r#"{"data":[{"id":"gpt-5"}]}"#),
    ]);
    let c = Client::new(&u, "sk-c", ApiType::OpenAiChat)
        .with_headers([("x-tenant".into(), "demo".into())].into());
    let (mut t, mut r, mut code) = (String::new(), String::new(), String::new());
    let rep = c
        .complete(
            &rich("oai", "gpt-5", Some("max"), None),
            0,
            &AtomicBool::new(false),
            &mut |d| match d {
                Delta::Text(s) => t.push_str(s),
                Delta::Reasoning(s) => r.push_str(s),
                Delta::Tool { code: k, .. } => code.push_str(k),
            },
            &mut |_| {},
        )
        .unwrap();
    assert!(t == "hi" && r == "ab" && code == "p()" && c.models().unwrap() == ["gpt-5"]);
    assert_eq!(
        rep.usage,
        Some(Usage {
            input: 100,
            output: 20,
            cached: 40,
            cache_write: 10
        })
    );
    let l = log.lock().unwrap();
    assert_eq!(l[0].1["x-tenant"], "demo");
    assert_eq!(l[1].1["x-tenant"], "demo");
    assert!(
        l[0].0 == "/chat/completions"
            && l[0].1["authorization"] == "Bearer sk-c"
            && pv(&l[0].2)["reasoning_effort"] == "max"
    );

    let (u, log) = serve(vec![
        sse(&[
            r#"{"type":"response.reasoning_summary_text.delta","delta":"plan"}"#,
            r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"rs_1","type":"reasoning","summary":[{"text":"plan"}],"encrypted_content":"ENC"}}"#,
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"fc_1","type":"function_call","call_id":"call_42","name":"lua","arguments":""}}"#,
            r#"{"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":1,"delta":"{\"code\":\"x=1\"}"}"#,
            r#"{"type":"response.function_call_arguments.done","item_id":"fc_1","output_index":1,"arguments":"{\"code\":\"x=2\"}"}"#,
            r#"{"type":"response.output_item.done","output_index":1,"item":{"id":"fc_1","type":"function_call"}}"#,
            r#"{"type":"response.completed","response":{"status":"completed","usage":{"input_tokens":80,"output_tokens":30,"input_tokens_details":{"cached_tokens":15}}}}"#,
        ]),
        j(
            r#"{"status":"completed","output":[{"id":"m2","type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}]}"#,
        ),
    ]);
    let c = Client::new(&u, "sk-r", ApiType::OpenAiResponses);
    let r1 = run(&c, &rich("resp", "gpt-5", Some("xhigh"), None)).unwrap();
    let rp = r1.replay.clone().unwrap();
    assert!(
        r1.reasoning == "plan"
            && r1.tool_calls[0].id == "call_42"
            && r1.tool_calls[0].arguments == r#"{"code":"x=2"}"#
    );
    assert!(
        rp.items[1]["call_id"] == "call_42"
            && rp.items[1]["arguments"] == r#"{"code":"x=2"}"#
            && r1.usage.unwrap().cached == 15
    );
    assert_eq!(
        run(&c, &rich("resp", "gpt-5", Some("high"), Some(rp)))
            .unwrap()
            .text,
        "done"
    );
    let l = log.lock().unwrap();
    assert!(pv(&l[0].2)["store"] == false && l[1].2.contains("ENC"));

    let (u, log) = serve(vec![
        sse(&[
            r#"{"type":"message_start","message":{"usage":{"input_tokens":100,"cache_read_input_tokens":50,"cache_creation_input_tokens":25}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"deep"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"SIG"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tu_1","name":"lua","input":{}}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"code\":\"a()\"}"}}"#,
            r#"{"type":"content_block_stop","index":1}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":42}}"#,
            r#"{"type":"message_stop"}"#,
        ]),
        j(r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"ok"}]}"#),
        j(r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"other"}]}"#),
        j(r#"{"data":[{"id":"a"}],"has_more":true,"last_id":"dup"}"#),
        j(r#"{"data":[{"id":"b"}],"has_more":true,"last_id":"dup"}"#),
    ]);
    let c = Client::new(&u, "ak", ApiType::AnthropicMessages);
    let r1 = run(&c, &rich("anth1", "claude-4", Some("max"), None)).unwrap();
    assert_eq!(
        r1.usage,
        Some(Usage {
            input: 175,
            output: 42,
            cached: 50,
            cache_write: 25
        })
    );
    let rp = r1.replay.clone().unwrap();
    run(
        &c,
        &rich("anth1", "claude-4", Some("high"), Some(rp.clone())),
    )
    .unwrap();
    run(&c, &rich("anth2", "claude-4", Some("high"), Some(rp))).unwrap();
    assert_eq!(c.models().unwrap(), ["a", "b"]);
    let l = log.lock().unwrap();
    assert!(pv(&l[0].2).get("thinking").is_none() && l[0].1["anthropic-version"] == "2023-06-01");
    assert!(
        pv(&l[1].2)["thinking"]["type"] == "adaptive"
            && l[1].2.contains("SIG")
            && !l[2].2.contains("SIG")
    );

    let (u, log) = serve(vec![
        sse(&[
            r#"{"candidates":[{"content":{"parts":[{"text":"t1","thought":true}]}}]}"#,
            r#"{"candidates":[{"content":{"parts":[{"text":"t2","thought":true,"thoughtSignature":"ST"},{"functionCall":{"name":"lua","args":{"code":"a()"}},"thoughtSignature":"SF"},{"functionCall":{"name":"lua","args":{"code":"b()"}}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":90,"cachedContentTokenCount":30,"candidatesTokenCount":10,"thoughtsTokenCount":15}}"#,
        ]),
        j(r#"[{"candidates":[{"content":{"parts":[{"text":"ab"}]},"finishReason":"STOP"}]}]"#),
        j(r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}]}"#),
        j(
            r#"{"models":[{"name":"models/gemini-1","supportedGenerationMethods":["generateContent"]},{"name":"models/embed","supportedGenerationMethods":["embedContent"]}]}"#,
        ),
    ]);
    let c = Client::new(&u, "gk", ApiType::GoogleGemini);
    let g1 = run(&c, &rich("gem", "gemini-2.5-pro", Some("medium"), None)).unwrap();
    assert!(
        g1.usage.unwrap().output == 25
            && g1.tool_calls[0].id == "call_0"
            && g1.tool_calls[1].id == "call_1"
    );
    let mut idless = ureq("gem", "gemini-2.5-pro", Some("high"));
    idless.messages.extend([
        Message::Assistant {
            text: "".into(),
            tool_calls: g1.tool_calls,
            replay: g1.replay,
        },
        Message::ToolResult {
            id: "call_0".into(),
            name: "lua".into(),
            output: "A".into(),
            images: None,
        },
        Message::ToolResult {
            id: "call_1".into(),
            name: "lua".into(),
            output: "B".into(),
            images: None,
        },
    ]);
    assert_eq!(run(&c, &idless).unwrap().text, "ab");
    run(&c, &rich("gem", "gemini-3-pro", Some("low"), None)).unwrap();
    assert_eq!(c.models().unwrap(), ["gemini-1"]);
    let l = log.lock().unwrap();
    assert!(!l[0].0.contains("gk") && l[0].1["x-goog-api-key"] == "gk");
    assert_eq!(
        pv(&l[0].2)["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        8192
    );
    assert!(
        pv(&l[1].2)["contents"][2]["parts"][0]["functionResponse"]
            .get("id")
            .is_none()
    );
    assert!(
        pv(&l[1].2)["contents"][2]["parts"][1]["functionResponse"]
            .get("id")
            .is_none()
    );
    let b2 = pv(&l[2].2);
    assert!(
        b2["generationConfig"]["thinkingConfig"]["thinkingLevel"] == "low"
            && b2["contents"][1]["parts"][1]["thoughtSignature"]
                == "skip_thought_signature_validator"
    );
}

#[test]
fn redirects_truncation_validation_and_cancel() {
    let (sink_u, sink_l) = serve(vec![j("{}"); 16]);
    let redir = format!("HTTP/1.1 307 X\r\nLocation: {sink_u}/steal\r\nConnection: close\r\n\r\n");
    let (front_u, _) = serve(vec![redir; 16]);
    for api in [
        ApiType::OpenAiChat,
        ApiType::OpenAiResponses,
        ApiType::AnthropicMessages,
        ApiType::GoogleGemini,
    ] {
        let c = Client::new(&front_u, "secret", api);
        assert!(matches!(c.models(), Err(Error::Status(307, _))));
        let e = run(&c, &ureq("p", "m", None)).unwrap_err();
        assert!(matches!(e, Error::Status(307, _)) && !e.retryable());
    }
    assert!(sink_l.lock().unwrap().is_empty());

    let trunc = [
        (
            ApiType::OpenAiChat,
            r#"{"choices":[{"delta":{"content":"x"}}]}"#,
        ),
        (
            ApiType::OpenAiResponses,
            r#"{"type":"response.output_text.delta","delta":"x"}"#,
        ),
        (
            ApiType::AnthropicMessages,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":"x"}}"#,
        ),
        (
            ApiType::GoogleGemini,
            r#"{"candidates":[{"content":{"parts":[{"text":"x"}]}}]}"#,
        ),
    ];
    for (api, f) in trunc {
        let (u, _) = serve(vec![sse(&[f])]);
        let e = run(&Client::new(&u, "k", api), &ureq("p", "m", None)).unwrap_err();
        assert!(matches!(e, Error::Transport(_)) && e.retryable(), "{api:?}");
    }

    let (unused_u, unused_l) = serve(vec![j("{}")]);
    for (api, ef) in [
        (ApiType::OpenAiResponses, "max"),
        (ApiType::AnthropicMessages, "xhigh"),
        (ApiType::GoogleGemini, "xhigh"),
        (ApiType::GoogleGemini, "max"),
    ] {
        assert!(matches!(
            run(&Client::new(&unused_u, "k", api), &ureq("p", "m", Some(ef))),
            Err(Error::Unsupported(_))
        ));
    }
    for bad in [
        "https://x/a.png",
        "data:image/png;base64,",
        "data:image/png;base64,ABC",
        "data:image/png;base64,not!base",
        "data:image/png;base64,AB==",
        "data:image/png;base64,AAB=",
    ] {
        let mut r = ureq("p", "m", None);
        r.messages = vec![Message::User {
            text: "x".into(),
            images: vec![bad.into()],
        }];
        assert!(
            matches!(
                run(&Client::new(&unused_u, "k", ApiType::GoogleGemini), &r),
                Err(Error::Unsupported(_))
            ),
            "{bad}"
        );
    }
    assert!(unused_l.lock().unwrap().is_empty());

    let (stall_u, _) = serve(vec![format!(
        "STALL:{}",
        sse(&[r#"{"candidates":[{"content":{"parts":[{"text":"hi"}]}}]}"#])
    )]);
    let flag = Arc::new(AtomicBool::new(false));
    let f2 = flag.clone();
    let e = Client::new(&stall_u, "k", ApiType::GoogleGemini)
        .complete(
            &ureq("g", "gemini-2.5-pro", None),
            0,
            &flag,
            &mut |_| f2.store(true, Ordering::Relaxed),
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(e, Error::Cancelled));
}
#[test]
fn responses_returns_at_terminal_event_without_eof() {
    use std::sync::mpsc;
    use std::time::Duration;

    for status in ["completed", "incomplete"] {
        let frame = serde_json::json!({
            "type": format!("response.{status}"),
            "response": {"status": status, "usage": {"input_tokens": 6, "output_tokens": 3}}
        });
        let (url, _) = serve(vec![format!(
            "STALL:{}",
            sse(&[
                r#"{"type":"response.output_text.delta","delta":"hello"}"#,
                &frame.to_string(),
            ])
        )]);
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let client = std::thread::spawn(move || {
            let reply = Client::new(&url, "key", ApiType::OpenAiResponses).complete(
                &ureq("resp", "gpt-5", None),
                0,
                &stop,
                &mut |_| {},
                &mut |_| {},
            );
            let _ = tx.send(reply);
        });
        let result = rx.recv_timeout(Duration::from_secs(2));
        cancel.store(true, Ordering::Relaxed);
        client.join().unwrap();
        let reply = result
            .expect("adapter waited for the HTTP body to close")
            .unwrap();
        assert_eq!(reply.text, "hello");
        assert_eq!(reply.finish, status);
        let usage = reply.usage.unwrap();
        assert_eq!((usage.input, usage.output), (6, 3));
    }
}
