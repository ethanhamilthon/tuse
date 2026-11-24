use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tuse_api::{
    ApiType, Client, Delta, Error, Message, Replay, Request, ToolCall, ToolDef, ToolImage, Usage,
};

#[derive(Clone, Debug)]
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
            let body = v.to_string();
            let _ = write!(
                out,
                "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
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

fn rich_request(
    provider: &str,
    model: &str,
    effort: Option<&str>,
    replay: Option<Replay>,
) -> Request {
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
                text: "also this".into(),
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

#[test]
fn openai_chat_encoding_streaming_fallback_and_models() {
    let srv = Server::start(vec![
        MockReply::Sse(vec![
            format!(
                "data: {}\n\n",
                json!({"choices":[{"delta":{"reasoning_content":"think ","reasoning":"more"}}]})
            ),
            format!(
                "data: {}\n\n",
                json!({"choices":[{"delta":{"content":"hi"}}]})
            ),
            format!(
                "data: {}\n\n",
                json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"lua","arguments":"{\"co"}}]}}]})
            ),
            format!(
                "data: {}\n\n",
                json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"de\":\"print(1)\"}"}}]}}]})
            ),
            format!(
                "data: {}\n\n",
                json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":40,"cache_write_tokens":10}}})
            ),
            "data: [DONE]\n\n".into(),
        ]),
        MockReply::Json(
            200,
            json!({
                "choices":[{"message":{"content":"plain","reasoning":"r"},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":10,"completion_tokens":5}
            }),
        ),
        MockReply::Json(200, json!({"data":[{"id":"gpt-5"},{"id":"o3"}]})),
    ]);
    let client = Client::new(&srv.url, "sk-chat", ApiType::OpenAiChat);
    let cancel = AtomicBool::new(false);
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut code = String::new();
    let reply = client
        .complete(
            &rich_request("openai", "gpt-5", Some("max"), None),
            0,
            &cancel,
            &mut |d| match d {
                Delta::Text(t) => text.push_str(t),
                Delta::Reasoning(r) => reasoning.push_str(r),
                Delta::Tool { code: c, .. } => code.push_str(c),
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(
        (text.as_str(), reasoning.as_str(), code.as_str()),
        ("hi", "think more", "print(1)")
    );
    assert_eq!(reply.finish, "tool_calls");
    assert_eq!(
        reply.usage,
        Some(Usage {
            input: 100,
            output: 20,
            cached: 40,
            cache_write: 10,
        })
    );
    assert!(reply.replay.is_none());

    let fallback = client
        .complete(
            &Request {
                provider: "openai".into(),
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
    assert_eq!(
        (fallback.text.as_str(), fallback.reasoning.as_str()),
        ("plain", "r")
    );
    assert_eq!(client.models().unwrap(), vec!["gpt-5", "o3"]);

    let reqs = srv.captured();
    assert_eq!(
        (reqs[0].method.as_str(), reqs[0].path.as_str()),
        ("POST", "/chat/completions")
    );
    assert_eq!(reqs[2].method, "GET");
    assert_eq!(reqs[0].headers["authorization"], "Bearer sk-chat");
    let body0: Value = serde_json::from_str(&reqs[0].body).unwrap();
    assert_eq!(body0["reasoning_effort"], "max");
    assert_eq!(body0["messages"][0]["role"], "system");
    assert_eq!(body0["messages"][1]["content"][1]["type"], "image_url");
    assert_eq!(
        body0["messages"][5]["content"][0]["text"],
        "[Images from the tool results above]"
    );
    let body1: Value = serde_json::from_str(&reqs[1].body).unwrap();
    assert!(body1.get("tools").is_none());
    assert_eq!(body1["messages"].as_array().unwrap().len(), 1);
}

#[test]
fn openai_responses_encoding_streaming_replay_fallback_and_models() {
    let srv = Server::start(vec![
        MockReply::Sse(vec![
            format!(
                "data: {}\n\n",
                json!({"type":"response.reasoning_summary_text.delta","delta":"plan"})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.output_item.done","output_index":0,"item":{"id":"rs_1","type":"reasoning","summary":[{"type":"summary_text","text":"plan"}],"encrypted_content":"ENC_BLOB"}})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.output_item.done","output_index":1,"item":{"id":"rs_empty","type":"reasoning","summary":[{"type":"summary_text","text":"drop me"}]}})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.output_text.delta","delta":"running"})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.output_item.done","output_index":2,"item":{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"output_text","text":"running"}]}})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.output_item.added","output_index":3,"item":{"id":"fc_item","type":"function_call","call_id":"call_42","name":"lua","arguments":""}})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.function_call_arguments.delta","item_id":"fc_item","output_index":3,"delta":"{\"code\":\"x"})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.function_call_arguments.delta","item_id":"fc_item","output_index":3,"delta":" = 1\"}"})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.function_call_arguments.done","item_id":"fc_item","output_index":3,"arguments":"{\"code\":\"x = 1\"}"})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.output_item.done","output_index":3,"item":{"id":"fc_item","type":"function_call","call_id":"call_42","name":"lua","arguments":"{\"code\":\"x = 1\"}"}})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.output_item.added","output_index":4,"item":{"id":"fc_2","type":"function_call","call_id":"call_99","name":"lua","arguments":""}})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.output_item.done","output_index":4,"item":{"id":"fc_2","type":"function_call","call_id":"call_99","name":"lua","arguments":"{\"code\":\"y = 2\"}"}})
            ),
            format!(
                "data: {}\n\n",
                json!({"type":"response.completed","response":{"status":"completed","usage":{"input_tokens":80,"output_tokens":30,"input_tokens_details":{"cached_tokens":15}}}})
            ),
        ]),
        MockReply::Json(
            200,
            json!({
                "status":"completed",
                "usage":{"input_tokens":50,"output_tokens":12,"input_tokens_details":{"cached_tokens":5}},
                "output":[
                    {"id":"rs_2","type":"reasoning","summary":[{"text":"thought"}],"encrypted_content":"ENC2"},
                    {"id":"msg_2","type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]},
                    {"id":"fc_fb","type":"function_call","call_id":"c_fb","name":"lua","arguments":"{\"code\":\"ok()\"}"}
                ]
            }),
        ),
        MockReply::Json(200, json!({"data":[{"id":"gpt-5-codex"}]})),
    ]);
    let client = Client::new(&srv.url, "sk-resp", ApiType::OpenAiResponses);
    let cancel = AtomicBool::new(false);
    let mut codes: Vec<(usize, String)> = vec![];
    let mut reasoning = String::new();
    let r1 = client
        .complete(
            &rich_request("resp", "gpt-5", Some("xhigh"), None),
            0,
            &cancel,
            &mut |d| match d {
                Delta::Reasoning(t) => reasoning.push_str(t),
                Delta::Tool { index, code, .. } if !code.is_empty() => {
                    codes.push((index, code.to_string()));
                }
                _ => {}
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(reasoning, "plan");
    let c0: String = codes
        .iter()
        .filter(|(i, _)| *i == 0)
        .map(|(_, c)| c.as_str())
        .collect();
    let c1: String = codes
        .iter()
        .filter(|(i, _)| *i == 1)
        .map(|(_, c)| c.as_str())
        .collect();
    assert_eq!((c0.as_str(), c1.as_str()), ("x = 1", "y = 2"));
    assert_eq!(r1.tool_calls[0].id, "call_42");
    assert_eq!(r1.tool_calls[1].id, "call_99");
    assert_eq!(
        r1.usage,
        Some(Usage {
            input: 80,
            output: 30,
            cached: 15,
            cache_write: 0,
        })
    );
    let rep = r1.replay.clone().unwrap();
    assert_eq!(rep.items.len(), 4);
    assert_eq!(rep.items[0]["encrypted_content"], "ENC_BLOB");
    assert!(rep.items.iter().all(|it| it["id"] != "rs_empty"));

    let mut fb_code = String::new();
    let r2 = client
        .complete(
            &rich_request("resp", "gpt-5", Some("high"), Some(rep)),
            0,
            &cancel,
            &mut |d| {
                if let Delta::Tool { code, .. } = d {
                    fb_code.push_str(code);
                }
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(
        (r2.text.as_str(), r2.reasoning.as_str(), fb_code.as_str()),
        ("done", "thought", "ok()")
    );
    assert_eq!(r2.replay.as_ref().unwrap().items.len(), 3);
    assert_eq!(client.models().unwrap(), vec!["gpt-5-codex"]);

    let reqs = srv.captured();
    assert_eq!(reqs[0].path, "/responses");
    let b0: Value = serde_json::from_str(&reqs[0].body).unwrap();
    assert_eq!(b0["store"], false);
    assert_eq!(b0["instructions"], "Be concise.");
    assert_eq!(
        b0["reasoning"],
        json!({"effort": "xhigh", "summary": "auto"})
    );
    assert_eq!(b0["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(b0["tools"][0]["name"], "lua");
    let b1: Value = serde_json::from_str(&reqs[1].body).unwrap();
    assert!(
        b1["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|it| it["encrypted_content"] == "ENC_BLOB")
    );
}

#[test]
fn anthropic_encoding_streaming_replay_fallback_and_pagination() {
    let mut model_replies = vec![];
    for p in 1..=11 {
        model_replies.push(MockReply::Json(
            200,
            json!({
                "data": [{"id": format!("claude-{p}")}],
                "has_more": true,
                "last_id": format!("cur-{p}"),
            }),
        ));
    }
    let srv_models = Server::start(model_replies);
    let ids = Client::new(&srv_models.url, "ak-1", ApiType::AnthropicMessages)
        .models()
        .unwrap();
    assert_eq!(ids.len(), 10);
    assert_eq!(ids[0], "claude-1");
    assert_eq!(ids[9], "claude-10");
    let mreqs = srv_models.captured();
    assert_eq!(mreqs.len(), 10);
    assert_eq!(mreqs[0].headers["x-api-key"], "ak-1");
    assert_eq!(mreqs[0].headers["anthropic-version"], "2023-06-01");
    assert_eq!(mreqs[0].path, "/models?limit=100");
    assert_eq!(mreqs[1].path, "/models?limit=100&after_id=cur-1");

    let srv_loop = Server::start(vec![
        MockReply::Json(
            200,
            json!({"data":[{"id":"a"}],"has_more":true,"last_id":"same"}),
        ),
        MockReply::Json(
            200,
            json!({"data":[{"id":"b"}],"has_more":true,"last_id":"same"}),
        ),
    ]);
    assert_eq!(
        Client::new(&srv_loop.url, "ak-1", ApiType::AnthropicMessages)
            .models()
            .unwrap(),
        vec!["a", "b"]
    );

    let srv = Server::start(vec![
        MockReply::Sse(vec![
            format!(
                "event: message_start\ndata: {}\n\n",
                json!({"type":"message_start","message":{"usage":{"input_tokens":100,"cache_read_input_tokens":50,"cache_creation_input_tokens":25,"output_tokens":1}}})
            ),
            format!(
                "event: content_block_start\ndata: {}\n\n",
                json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}})
            ),
            format!(
                "event: content_block_delta\ndata: {}\n\n",
                json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"deep"}})
            ),
            format!(
                "event: content_block_delta\ndata: {}\n\n",
                json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"SIG_A"}})
            ),
            format!(
                "event: content_block_delta\ndata: {}\n\n",
                json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"_B"}})
            ),
            format!(
                "event: content_block_stop\ndata: {}\n\n",
                json!({"type":"content_block_stop","index":0})
            ),
            format!(
                "event: content_block_start\ndata: {}\n\n",
                json!({"type":"content_block_start","index":1,"content_block":{"type":"redacted_thinking","data":"OPAQUE"}})
            ),
            format!(
                "event: content_block_stop\ndata: {}\n\n",
                json!({"type":"content_block_stop","index":1})
            ),
            format!(
                "event: content_block_start\ndata: {}\n\n",
                json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"tu_1","name":"lua","input":{}}})
            ),
            format!(
                "event: content_block_delta\ndata: {}\n\n",
                json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"code\":\"a"}})
            ),
            format!(
                "event: content_block_delta\ndata: {}\n\n",
                json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":" + b\"}"}})
            ),
            format!(
                "event: content_block_stop\ndata: {}\n\n",
                json!({"type":"content_block_stop","index":2})
            ),
            format!(
                "event: message_delta\ndata: {}\n\n",
                json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":42}})
            ),
            format!("event: ping\ndata: {}\n\n", json!({"type":"ping"})),
            format!(
                "event: message_stop\ndata: {}\n\n",
                json!({"type":"message_stop"})
            ),
        ]),
        MockReply::Json(
            200,
            json!({
                "stop_reason":"end_turn",
                "usage":{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":5,"output_tokens":7},
                "content":[
                    {"type":"thinking","thinking":"fb","signature":"S"},
                    {"type":"redacted_thinking","data":"R"},
                    {"type":"text","text":"answer"},
                    {"type":"tool_use","id":"tu_2","name":"lua","input":{"code":"z()"}}
                ]
            }),
        ),
        MockReply::Json(
            200,
            json!({
                "stop_reason":"end_turn",
                "content":[{"type":"text","text":"recovered"}]
            }),
        ),
    ]);
    let client = Client::new(&srv.url, "ak-test", ApiType::AnthropicMessages);
    let cancel = AtomicBool::new(false);
    let mut reasoning = String::new();
    let mut code = String::new();
    let r1 = client
        .complete(
            &Request {
                provider: "anth".into(),
                model: "claude-sonnet-4".into(),
                system: "sys".into(),
                messages: vec![
                    Message::User {
                        text: "u1".into(),
                        images: vec!["data:image/png;base64,QUFB".into()],
                    },
                    Message::User {
                        text: "u2".into(),
                        images: vec![],
                    },
                ],
                tools: vec![lua_tool()],
                effort: Some("max".into()),
            },
            0,
            &cancel,
            &mut |d| match d {
                Delta::Reasoning(t) => reasoning.push_str(t),
                Delta::Tool { code: c, .. } => code.push_str(c),
                _ => {}
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!((reasoning.as_str(), code.as_str()), ("deep", "a + b"));
    assert_eq!(r1.finish, "tool_use");
    assert_eq!(
        r1.usage,
        Some(Usage {
            input: 175,
            output: 42,
            cached: 50,
            cache_write: 25,
        })
    );
    let rep = r1.replay.clone().unwrap();
    assert_eq!(
        rep.items[0],
        json!({"type":"thinking","thinking":"deep","signature":"SIG_A_B"})
    );
    assert_eq!(
        rep.items[1],
        json!({"type":"redacted_thinking","data":"OPAQUE"})
    );
    assert_eq!(rep.items[2]["type"], "tool_use");

    let mut fb_code = String::new();
    let r2 = client
        .complete(
            &rich_request("anth", "claude-sonnet-4", Some("high"), Some(rep)),
            0,
            &cancel,
            &mut |d| {
                if let Delta::Tool { code: c, .. } = d {
                    fb_code.push_str(c);
                }
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(
        (r2.text.as_str(), r2.reasoning.as_str(), fb_code.as_str()),
        ("answer", "fb", "z()")
    );
    assert_eq!(r2.usage.unwrap().input, 35);

    client
        .complete(
            &rich_request("anth", "claude-sonnet-4", Some("high"), None),
            0,
            &cancel,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();

    let reqs = srv.captured();
    let b0: Value = serde_json::from_str(&reqs[0].body).unwrap();
    assert_eq!(b0["max_tokens"], 16384);
    assert_eq!(
        b0["system"][0]["cache_control"],
        json!({"type":"ephemeral"})
    );
    assert_eq!(b0["messages"].as_array().unwrap().len(), 1);
    assert_eq!(
        b0["messages"][0]["content"][2]["cache_control"],
        json!({"type":"ephemeral"})
    );
    assert_eq!(b0["thinking"], json!({"type":"adaptive"}));
    assert_eq!(b0["output_config"], json!({"effort":"max"}));

    let b1: Value = serde_json::from_str(&reqs[1].body).unwrap();
    assert_eq!(b1["messages"][1]["content"][0]["signature"], "SIG_A_B");
    assert_eq!(b1["messages"][1]["content"][1]["type"], "redacted_thinking");
    assert_eq!(b1["messages"][2]["content"][0]["type"], "tool_result");
    assert_eq!(
        b1["messages"][2]["content"][0]["content"][1]["type"],
        "image"
    );
    assert_eq!(b1["thinking"], json!({"type":"adaptive"}));

    let b2: Value = serde_json::from_str(&reqs[2].body).unwrap();
    assert!(b2.get("thinking").is_none());
    assert!(b2.get("output_config").is_none());
}

#[test]
fn gemini_encoding_streaming_replay_fallback_and_pagination() {
    let mut model_replies = vec![];
    for p in 1..=11 {
        model_replies.push(MockReply::Json(
            200,
            json!({
                "models": [
                    {"name": format!("models/gemini-{p}"), "supportedGenerationMethods": ["generateContent"]},
                    {"name": format!("models/embed-{p}"), "supportedGenerationMethods": ["embedContent"]},
                ],
                "nextPageToken": format!("tok-{p}"),
            }),
        ));
    }
    let srv_models = Server::start(model_replies);
    let ids = Client::new(&srv_models.url, "gk-1", ApiType::GoogleGemini)
        .models()
        .unwrap();
    assert_eq!(ids.len(), 10);
    assert_eq!(ids[0], "gemini-1");
    assert!(
        ids.iter()
            .all(|id| !id.starts_with("models/") && !id.contains("embed"))
    );
    let mreqs = srv_models.captured();
    assert_eq!(mreqs[0].headers["x-goog-api-key"], "gk-1");
    assert!(!mreqs[0].path.contains("gk-1"));
    assert_eq!(mreqs[1].path, "/models?pageSize=100&pageToken=tok-1");

    let srv = Server::start(vec![
        MockReply::Sse(vec![
            format!(
                "data: {}\n\n",
                json!({"candidates":[{"content":{"parts":[{"text":"t1","thought":true}]}}]})
            ),
            format!(
                "data: {}\n\n",
                json!({"candidates":[{"content":{"parts":[{"text":"t2","thought":true}]}}]})
            ),
            format!(
                "data: {}\n\n",
                json!({"candidates":[{"content":{"parts":[{"text":"t3","thought":true,"thoughtSignature":"SIG_T"}]}}]})
            ),
            format!(
                "data: {}\n\n",
                json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"lua","args":{"code":"return 7"}},"thoughtSignature":"SIG_FC"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":90,"cachedContentTokenCount":30,"candidatesTokenCount":10,"thoughtsTokenCount":15}})
            ),
        ]),
        MockReply::Json(
            200,
            json!([
                {"candidates":[{"content":{"parts":[{"text":"hello "}]}}]},
                {"candidates":[{"content":{"parts":[{"text":"world"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":20,"candidatesTokenCount":4,"thoughtsTokenCount":6}}
            ]),
        ),
        MockReply::Json(
            200,
            json!({"candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}]}),
        ),
    ]);
    let client = Client::new(&srv.url, "gk-secret", ApiType::GoogleGemini);
    let cancel = AtomicBool::new(false);
    let mut reasoning = String::new();
    let mut code = String::new();
    let r1 = client
        .complete(
            &rich_request("gem", "gemini-2.5-pro", Some("medium"), None),
            0,
            &cancel,
            &mut |d| match d {
                Delta::Reasoning(t) => reasoning.push_str(t),
                Delta::Tool { code: c, .. } => code.push_str(c),
                _ => {}
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!((reasoning.as_str(), code.as_str()), ("t1t2t3", "return 7"));
    assert_eq!(r1.tool_calls[0].id, "call_0");
    assert_eq!(
        r1.usage,
        Some(Usage {
            input: 90,
            output: 25,
            cached: 30,
            cache_write: 0,
        })
    );
    let rep = r1.replay.clone().unwrap();
    assert_eq!(rep.items.len(), 3);
    assert_eq!(rep.items[0], json!({"text":"t1t2","thought":true}));
    assert_eq!(rep.items[1]["thoughtSignature"], "SIG_T");
    assert_eq!(rep.items[2]["thoughtSignature"], "SIG_FC");

    let r2 = client
        .complete(
            &rich_request("gem", "gemini-2.5-pro", Some("high"), Some(rep)),
            0,
            &cancel,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(r2.text, "hello world");
    assert_eq!(r2.usage.unwrap().output, 10);

    client
        .complete(
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

    let reqs = srv.captured();
    assert_eq!(
        reqs[0].path,
        "/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
    );
    assert_eq!(reqs[0].headers["x-goog-api-key"], "gk-secret");
    assert!(!reqs[0].path.contains("gk-secret"));
    let b0: Value = serde_json::from_str(&reqs[0].body).unwrap();
    assert_eq!(b0["systemInstruction"]["parts"][0]["text"], "Be concise.");
    assert_eq!(
        b0["generationConfig"]["thinkingConfig"],
        json!({"includeThoughts":true,"thinkingBudget":8192})
    );
    assert_eq!(b0["contents"][0]["parts"].as_array().unwrap().len(), 3);
    assert_eq!(
        b0["contents"][2]["parts"][0]["functionResponse"]["name"],
        "lua"
    );
    assert_eq!(
        b0["contents"][2]["parts"][1]["inlineData"]["mimeType"],
        "image/jpeg"
    );

    let b1: Value = serde_json::from_str(&reqs[1].body).unwrap();
    assert_eq!(b1["contents"][1]["parts"][1]["thoughtSignature"], "SIG_T");
    assert_eq!(b1["contents"][1]["parts"][2]["thoughtSignature"], "SIG_FC");

    let b2: Value = serde_json::from_str(&reqs[2].body).unwrap();
    assert_eq!(
        b2["generationConfig"]["thinkingConfig"],
        json!({"includeThoughts":true,"thinkingLevel":"low"})
    );
    assert!(b2.get("systemInstruction").is_none());
    assert!(b2.get("tools").is_none());
}

#[test]
fn unsupported_capabilities_and_cooperative_cancellation() {
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
        assert!(
            err.to_string()
                .contains(&format!("does not support effort `{effort}`")),
            "{err}"
        );
    }
    for api in [ApiType::AnthropicMessages, ApiType::GoogleGemini] {
        let err = Client::new(&srv.url, "k", api)
            .complete(
                &Request {
                    provider: "p".into(),
                    model: "m".into(),
                    system: String::new(),
                    messages: vec![Message::User {
                        text: "hi".into(),
                        images: vec!["https://example.com/a.png".into()],
                    }],
                    tools: vec![],
                    effort: None,
                },
                3,
                &cancel,
                &mut |_| {},
                &mut |_| panic!("must not retry"),
            )
            .unwrap_err();
        assert!(matches!(err, Error::Unsupported(_)));
        assert!(err.to_string().contains("does not support image"), "{err}");
    }
    assert!(
        srv.captured().is_empty(),
        "zero HTTP requests on unsupported capability"
    );

    let stall = Server::start(vec![MockReply::StallSse(
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}\n\n".into(),
    )]);
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
