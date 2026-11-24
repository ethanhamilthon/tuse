use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tuse_api::{
    ApiType, Client, Delta, Error, Message, Replay, Request, ToolCall, ToolDef, ToolImage, Usage,
};

#[derive(Clone)]
struct Captured {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Clone)]
enum MockReply {
    Json(u16, Value),
    Sse(Vec<String>),
    StallSse(String),
}

struct Server {
    url: String,
    seen: Arc<Mutex<Vec<Captured>>>,
}

impl Server {
    fn start(replies: Vec<MockReply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(vec![]));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (n, stream) in listener.incoming().flatten().enumerate() {
                let Some(reply) = replies.get(n).cloned() else {
                    break;
                };
                let log = log.clone();
                std::thread::spawn(move || handle(stream, reply, log));
            }
        });
        Self { url, seen }
    }

    fn captured(&self) -> Vec<Captured> {
        self.seen.lock().unwrap().clone()
    }
}

fn handle(stream: std::net::TcpStream, reply: MockReply, log: Arc<Mutex<Vec<Captured>>>) {
    let mut r = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    if r.read_line(&mut first).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::new();
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim().to_string();
            if k == "content-length" {
                len = v.parse().unwrap_or(0);
            }
            headers.insert(k, v);
        }
    }
    let mut buf = vec![0; len];
    if len > 0 {
        let _ = r.read_exact(&mut buf);
    }
    log.lock().unwrap().push(Captured {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&buf).into_owned(),
    });
    let mut out = stream;
    match reply {
        MockReply::Json(code, v) => {
            let b = v.to_string();
            let _ = write!(
                out,
                "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
                b.len()
            );
        }
        MockReply::Sse(frames) => {
            let _ = out.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            for f in frames {
                let _ = out.write_all(f.as_bytes());
                let _ = out.flush();
            }
        }
        MockReply::StallSse(first_frame) => {
            let _ = out.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            let _ = out.write_all(first_frame.as_bytes());
            let _ = out.flush();
            while out.write_all(b": ping\n\n").is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
    }
}

fn lua_tool() -> ToolDef {
    ToolDef {
        name: "lua".into(),
        description: "Run Lua".into(),
        parameters: json!({"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}),
    }
}

fn rich_req(provider: &str, model: &str, effort: Option<&str>, replay: Option<Replay>) -> Request {
    Request {
        provider: provider.into(),
        model: model.into(),
        system: "Be concise.".into(),
        messages: vec![
            Message::User {
                text: "look".into(),
                images: vec!["data:image/png;base64,QUFB".into()],
            },
            Message::User {
                text: "also".into(),
                images: vec![],
            },
            Message::Assistant {
                text: "checking".into(),
                tool_calls: vec![ToolCall {
                    id: "tc_1".into(),
                    name: "lua".into(),
                    arguments: r#"{"code":"return 1"}"#.into(),
                }],
                replay,
            },
            Message::ToolResult {
                id: "tc_1".into(),
                name: "lua".into(),
                output: "1".into(),
                images: Some(ToolImage {
                    label: "read('img.png')".into(),
                    urls: vec!["data:image/jpeg;base64,QkJC".into()],
                }),
            },
        ],
        tools: vec![lua_tool()],
        effort: effort.map(String::from),
    }
}

fn sse(v: Value) -> String {
    format!("data: {v}\n\n")
}

#[test]
fn openai_chat_and_responses_wire_contracts() {
    let chat_srv = Server::start(vec![
        MockReply::Sse(vec![
            sse(json!({"choices":[{"delta":{"reasoning_content":"a","reasoning":"b"}}]})),
            sse(json!({"choices":[{"delta":{"content":"hi"}}]})),
            sse(
                json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"lua","arguments":"{\"co"}}]}}]}),
            ),
            sse(
                json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"de\":\"p()\"}"}}]}}]}),
            ),
            sse(
                json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":40,"cache_write_tokens":10}}}),
            ),
            "data: [DONE]\n\n".into(),
        ]),
        MockReply::Json(
            200,
            json!({"choices":[{"message":{"content":"fb"},"finish_reason":"stop"}]}),
        ),
        MockReply::Json(200, json!({"data":[{"id":"gpt-5"}]})),
    ]);
    let chat = Client::new(&chat_srv.url, "sk-c", ApiType::OpenAiChat);
    let cancel = AtomicBool::new(false);
    let (mut t, mut r, mut c) = (String::new(), String::new(), String::new());
    let rep = chat
        .complete(
            &rich_req("oai", "gpt-5", Some("max"), None),
            0,
            &cancel,
            &mut |d| match d {
                Delta::Text(s) => t.push_str(s),
                Delta::Reasoning(s) => r.push_str(s),
                Delta::Tool { code, .. } => c.push_str(code),
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!((t.as_str(), r.as_str(), c.as_str()), ("hi", "ab", "p()"));
    assert_eq!(
        rep.usage,
        Some(Usage {
            input: 100,
            output: 20,
            cached: 40,
            cache_write: 10
        })
    );
    let fb = chat
        .complete(
            &Request {
                provider: "oai".into(),
                model: "gpt-5".into(),
                system: String::new(),
                messages: vec![Message::User {
                    text: "ping".into(),
                    images: vec![],
                }],
                tools: vec![],
                effort: None,
            },
            0,
            &cancel,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(fb.text, "fb");
    assert_eq!(chat.models().unwrap(), vec!["gpt-5"]);
    let reqs = chat_srv.captured();
    assert_eq!(
        (reqs[0].method.as_str(), reqs[0].path.as_str()),
        ("POST", "/chat/completions")
    );
    assert_eq!(reqs[0].headers["authorization"], "Bearer sk-c");
    assert_eq!(reqs[2].method, "GET");

    let resp_srv = Server::start(vec![
        MockReply::Sse(vec![
            sse(json!({"type":"response.reasoning_summary_text.delta","delta":"plan"})),
            sse(
                json!({"type":"response.output_item.done","output_index":0,"item":{"id":"rs_1","type":"reasoning","summary":[{"text":"plan"}],"encrypted_content":"ENC"}}),
            ),
            sse(
                json!({"type":"response.output_item.done","output_index":1,"item":{"id":"rs_drop","type":"reasoning","summary":[{"text":"x"}]}}),
            ),
            sse(
                json!({"type":"response.output_item.added","output_index":2,"item":{"id":"fc_1","type":"function_call","call_id":"call_42","name":"lua","arguments":""}}),
            ),
            sse(
                json!({"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":2,"delta":"{\"code\":\"x=1\"}"}),
            ),
            sse(
                json!({"type":"response.function_call_arguments.done","item_id":"fc_1","output_index":2,"arguments":"{\"code\":\"x=2\"}"}),
            ),
            sse(
                json!({"type":"response.output_item.done","output_index":2,"item":{"id":"fc_1","type":"function_call"}}),
            ),
            sse(
                json!({"type":"response.completed","response":{"status":"completed","usage":{"input_tokens":80,"output_tokens":30,"input_tokens_details":{"cached_tokens":15}}}}),
            ),
        ]),
        MockReply::Json(
            200,
            json!({
                "status":"completed",
                "output":[
                    {"id":"rs_2","type":"reasoning","summary":[{"text":"th"}],"encrypted_content":"E2"},
                    {"id":"msg_2","type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]},
                    {"id":"fc_2","type":"function_call","call_id":"c2","name":"lua","arguments":"{\"code\":\"ok()\"}"}
                ]
            }),
        ),
        MockReply::Json(200, json!({"data":[{"id":"gpt-5-codex"}]})),
    ]);
    let resp = Client::new(&resp_srv.url, "sk-r", ApiType::OpenAiResponses);
    let mut rcode = String::new();
    let r1 = resp
        .complete(
            &rich_req("resp", "gpt-5", Some("xhigh"), None),
            0,
            &cancel,
            &mut |d| {
                if let Delta::Tool { code, .. } = d {
                    rcode.push_str(code);
                }
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(rcode, "x=1");
    assert_eq!(r1.tool_calls[0].id, "call_42");
    assert_eq!(r1.tool_calls[0].arguments, "{\"code\":\"x=2\"}");
    let replay = r1.replay.clone().unwrap();
    assert_eq!(replay.provider, "resp");
    assert_eq!(replay.items.len(), 2);
    assert_eq!(replay.items[1]["call_id"], "call_42");
    assert_eq!(replay.items[1]["arguments"], "{\"code\":\"x=2\"}");
    let r2 = resp
        .complete(
            &rich_req("resp", "gpt-5", Some("high"), Some(replay)),
            0,
            &cancel,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(r2.text, "done");
    assert_eq!(resp.models().unwrap(), vec!["gpt-5-codex"]);
    let rreqs = resp_srv.captured();
    let b0: Value = serde_json::from_str(&rreqs[0].body).unwrap();
    assert_eq!(b0["store"], false);
    assert_eq!(b0["include"], json!(["reasoning.encrypted_content"]));
    let b1: Value = serde_json::from_str(&rreqs[1].body).unwrap();
    assert!(
        b1["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|it| it["encrypted_content"] == "ENC")
    );
}

#[test]
fn anthropic_and_gemini_wire_contracts_and_pagination() {
    let mut amodels = vec![];
    for p in 1..=11 {
        amodels.push(MockReply::Json(
            200,
            json!({"data":[{"id":format!("c-{p}")}],"has_more":true,"last_id":format!("id-{p}")}),
        ));
    }
    let asrv_m = Server::start(amodels);
    let aids = Client::new(&asrv_m.url, "ak", ApiType::AnthropicMessages)
        .models()
        .unwrap();
    assert_eq!(
        (aids.len(), aids[0].as_str(), aids[9].as_str()),
        (10, "c-1", "c-10")
    );
    assert_eq!(
        asrv_m.captured()[0].headers["anthropic-version"],
        "2023-06-01"
    );

    let loop_m = Server::start(vec![
        MockReply::Json(
            200,
            json!({"data":[{"id":"a"}],"has_more":true,"last_id":"dup"}),
        ),
        MockReply::Json(
            200,
            json!({"data":[{"id":"b"}],"has_more":true,"last_id":"dup"}),
        ),
    ]);
    assert_eq!(
        Client::new(&loop_m.url, "ak", ApiType::AnthropicMessages)
            .models()
            .unwrap(),
        vec!["a", "b"]
    );

    let asrv = Server::start(vec![
        MockReply::Sse(vec![
            sse(
                json!({"type":"message_start","message":{"usage":{"input_tokens":100,"cache_read_input_tokens":50,"cache_creation_input_tokens":25}}}),
            ),
            sse(
                json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            ),
            sse(
                json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"deep"}}),
            ),
            sse(
                json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"SIG"}}),
            ),
            sse(json!({"type":"content_block_stop","index":0})),
            sse(
                json!({"type":"content_block_start","index":1,"content_block":{"type":"redacted_thinking","data":"RED"}}),
            ),
            sse(json!({"type":"content_block_stop","index":1})),
            sse(
                json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"tu_1","name":"lua","input":{}}}),
            ),
            sse(
                json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"code\":\"a()\"}"}}),
            ),
            sse(json!({"type":"content_block_stop","index":2})),
            sse(
                json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":42}}),
            ),
            sse(json!({"type":"message_stop"})),
        ]),
        MockReply::Json(
            200,
            json!({"stop_reason":"end_turn","usage":{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":5,"output_tokens":7},"content":[{"type":"text","text":"ans"}]}),
        ),
        MockReply::Json(
            200,
            json!({"stop_reason":"end_turn","content":[{"type":"text","text":"diff-prov"}]}),
        ),
    ]);
    let anth = Client::new(&asrv.url, "ak", ApiType::AnthropicMessages);
    let cancel = AtomicBool::new(false);
    let r1 = anth
        .complete(
            &rich_req("anth1", "claude-4", Some("max"), None),
            0,
            &cancel,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(
        r1.usage,
        Some(Usage {
            input: 175,
            output: 42,
            cached: 50,
            cache_write: 25
        })
    );
    let rep = r1.replay.clone().unwrap();
    anth.complete(
        &rich_req("anth1", "claude-4", Some("high"), Some(rep.clone())),
        0,
        &cancel,
        &mut |_| {},
        &mut |_| {},
    )
    .unwrap();
    anth.complete(
        &rich_req("anth2", "claude-4", Some("high"), Some(rep)),
        0,
        &cancel,
        &mut |_| {},
        &mut |_| {},
    )
    .unwrap();
    let areqs = asrv.captured();
    let ab0: Value = serde_json::from_str(&areqs[0].body).unwrap();
    assert!(ab0.get("thinking").is_none());
    let ab1: Value = serde_json::from_str(&areqs[1].body).unwrap();
    assert_eq!(ab1["messages"][1]["content"][0]["signature"], "SIG");
    assert_eq!(
        ab1["messages"][2]["content"][0]["content"][1]["type"],
        "image"
    );
    assert_eq!(ab1["thinking"], json!({"type":"adaptive"}));
    let ab2: Value = serde_json::from_str(&areqs[2].body).unwrap();
    assert!(
        !ab2.to_string().contains("SIG"),
        "same API/model different provider must not replay"
    );
    assert!(ab2.get("thinking").is_none());

    let mut gmodels = vec![];
    for p in 1..=11 {
        gmodels.push(MockReply::Json(
            200,
            json!({
                "models":[
                    {"name":format!("models/gemini-{p}"),"supportedGenerationMethods":["generateContent"]},
                    {"name":format!("models/embed-{p}"),"supportedGenerationMethods":["embedContent"]}
                ],
                "nextPageToken":format!("tok-{p}")
            }),
        ));
    }
    let gsrv_m = Server::start(gmodels);
    let gids = Client::new(&gsrv_m.url, "gk", ApiType::GoogleGemini)
        .models()
        .unwrap();
    assert_eq!((gids.len(), gids[0].as_str()), (10, "gemini-1"));
    assert!(!gsrv_m.captured()[0].path.contains("gk"));

    let gsrv = Server::start(vec![
        MockReply::Sse(vec![
            sse(json!({"candidates":[{"content":{"parts":[{"text":"t1","thought":true}]}}]})),
            sse(
                json!({"candidates":[{"content":{"parts":[{"text":"t2","thought":true,"thoughtSignature":"ST"},{"functionCall":{"name":"lua","args":{"code":"r()"}},"thoughtSignature":"SF"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":90,"cachedContentTokenCount":30,"candidatesTokenCount":10,"thoughtsTokenCount":15}}),
            ),
        ]),
        MockReply::Json(
            200,
            json!([{"candidates":[{"content":{"parts":[{"text":"a"}]}}]},{"candidates":[{"content":{"parts":[{"text":"b"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":20,"candidatesTokenCount":4,"thoughtsTokenCount":6}}]),
        ),
        MockReply::Json(
            200,
            json!({"candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}]}),
        ),
    ]);
    let gem = Client::new(&gsrv.url, "gk", ApiType::GoogleGemini);
    let gr1 = gem
        .complete(
            &rich_req("gem", "gemini-2.5-pro", Some("medium"), None),
            0,
            &cancel,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(gr1.usage.unwrap().output, 25);
    let gr2 = gem
        .complete(
            &rich_req("gem", "gemini-2.5-pro", Some("high"), gr1.replay),
            0,
            &cancel,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    assert_eq!((gr2.text.as_str(), gr2.usage.unwrap().output), ("ab", 10));
    gem.complete(
        &Request {
            provider: "gem".into(),
            model: "gemini-3-pro".into(),
            system: String::new(),
            messages: vec![Message::User {
                text: "hi".into(),
                images: vec![],
            }],
            tools: vec![],
            effort: Some("low".into()),
        },
        0,
        &cancel,
        &mut |_| {},
        &mut |_| {},
    )
    .unwrap();
    let greqs = gsrv.captured();
    assert!(!greqs[0].path.contains("gk"));
    assert_eq!(greqs[0].headers["x-goog-api-key"], "gk");
    let gb0: Value = serde_json::from_str(&greqs[0].body).unwrap();
    assert_eq!(
        gb0["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        8192
    );
    let gb2: Value = serde_json::from_str(&greqs[2].body).unwrap();
    assert_eq!(
        gb2["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        "low"
    );
}

#[test]
fn unsupported_capabilities_and_cancellation() {
    let srv = Server::start(vec![MockReply::Json(200, json!({}))]);
    let cancel = AtomicBool::new(false);
    for (api, effort) in [
        (ApiType::OpenAiResponses, "max"),
        (ApiType::AnthropicMessages, "xhigh"),
        (ApiType::GoogleGemini, "xhigh"),
        (ApiType::GoogleGemini, "max"),
    ] {
        let err = Client::new(&srv.url, "k", api)
            .complete(
                &Request {
                    provider: "p".into(),
                    model: "m".into(),
                    system: String::new(),
                    messages: vec![Message::User {
                        text: "hi".into(),
                        images: vec![],
                    }],
                    tools: vec![],
                    effort: Some(effort.into()),
                },
                3,
                &cancel,
                &mut |_| {},
                &mut |_| panic!("must not retry"),
            )
            .unwrap_err();
        assert!(matches!(err, Error::Unsupported(_)));
    }
    assert!(srv.captured().is_empty());

    let stall = Server::start(vec![MockReply::StallSse(sse(
        json!({"candidates":[{"content":{"parts":[{"text":"hi"}]}}]}),
    ))]);
    let cancel = Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone();
    let err = Client::new(&stall.url, "k", ApiType::GoogleGemini)
        .complete(
            &Request {
                provider: "g".into(),
                model: "gemini-2.5-pro".into(),
                system: String::new(),
                messages: vec![Message::User {
                    text: "hi".into(),
                    images: vec![],
                }],
                tools: vec![],
                effort: None,
            },
            0,
            &cancel,
            &mut |_| c2.store(true, Ordering::Relaxed),
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(err, Error::Cancelled));
}
