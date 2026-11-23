use super::*;
use tuse_api::ToolCall;

#[test]
fn notification_history_keeps_api_role() {
    let r = Record::Notification {
        text: "ready".into(),
    };
    assert_eq!(serde_json::to_value(&r).unwrap()["t"], "notification");
    assert_eq!(
        full(&r, false),
        Some(json!({"role":"user", "content":"ready"}))
    );
}

fn user(t: &str, images: Vec<String>) -> Record {
    Record::User {
        text: t.into(),
        context: String::new(),
        images,
        at: String::new(),
    }
}

fn call(id: &str) -> Record {
    Record::Assistant {
        text: String::new(),
        reasoning: String::new(),
        tool_calls: vec![ToolCall {
            id: id.into(),
            name: "lua".into(),
            arguments: r#"{"code":"read('a.png')"}"#.into(),
        }],
        at: String::new(),
    }
}

fn result(id: &str, images: Vec<String>) -> Record {
    Record::Tool {
        id: id.into(),
        tool: "lua".into(),
        input: "read('a.png')".into(),
        output: "ok".into(),
        code: Some(0),
        images,
        diffs: vec![],
        at: String::new(),
    }
}

/// Every `tool_call` is answered right after its message, before any `user` message.
fn valid(msgs: &[Value]) -> bool {
    let mut open: Vec<String> = vec![];
    for m in msgs {
        match m["role"].as_str() {
            Some("tool") => open.retain(|id| m["tool_call_id"] != id.as_str()),
            _ if !open.is_empty() => return false,
            _ => {}
        }
        if let Some(calls) = m["tool_calls"].as_array() {
            open.extend(calls.iter().map(|c| c["id"].as_str().unwrap().to_string()));
        }
    }
    open.is_empty()
}

#[test]
fn messages_keep_tool_results_with_their_calls() {
    let png = vec!["data:image/png;base64,AA".to_string()];
    let records = vec![
        user("one", vec![]),
        call("c1"),
        Record::Notice { text: "n".into() },
        result("c1", png.clone()),
        Record::Hook {
            name: "h".into(),
            event: "agent_message".into(),
            vars: String::new(),
            error: None,
            ms: 1,
        },
        user("two", png),
        call("c2"),
        result("c2", vec![]),
    ];
    for drop in 0..=records.len() {
        for prune in drop..=records.len() {
            let msgs = messages(&records, Bounds { prune, drop }, "s");
            assert!(valid(&msgs), "drop {drop} prune {prune}: {msgs:#?}");
        }
    }
    // The tool's image follows its result as a user message, before the next user text.
    let msgs = messages(&records, Bounds::default(), "s");
    let roles: Vec<_> = msgs.iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(
        roles,
        [
            "system",
            "user",
            "assistant",
            "tool",
            "user",
            "user",
            "assistant",
            "tool"
        ]
    );
    assert_eq!(
        msgs[4]["content"][0]["text"],
        "[Images from the tool results above]"
    );
    assert_eq!(msgs[5]["content"][1]["type"], "image_url");
}

#[test]
fn a_crash_mid_step_gets_placeholder_results() {
    let mut records = vec![user("go", vec![]), call("c1")];
    let Record::Assistant { tool_calls, .. } = &mut records[1] else {
        unreachable!()
    };
    tool_calls.push(ToolCall {
        id: "c2".into(),
        name: "lua".into(),
        arguments: "not json".into(),
    });
    records.push(result("c1", vec![]));
    let missing = missing_results(&records);
    assert_eq!(missing.len(), 1);
    let Record::Tool {
        id, input, code, ..
    } = &missing[0]
    else {
        panic!()
    };
    assert_eq!(
        (id.as_str(), input.as_str(), *code),
        ("c2", "not json", None)
    );
    records.extend(missing);
    assert!(missing_results(&records).is_empty());
    assert!(valid(&messages(&records, Bounds::default(), "s")));
}

#[test]
fn context_rides_along_except_in_the_dropped_prefix() {
    let r = Record::User {
        text: "see #p".into(),
        context: "<prompt name=\"p\">body</prompt>".into(),
        images: vec![],
        at: String::new(),
    };
    let both = "see #p\n\n<prompt name=\"p\">body</prompt>";
    assert_eq!(full(&r, false).unwrap()["content"], both);
    assert_eq!(conversation_only(&r).unwrap()["content"], "see #p");
    assert_eq!(join_context("", "c"), "c");
}

#[test]
fn old_user_records_have_no_context() {
    let r: Record = serde_json::from_str(r#"{"t":"user","text":"hi","at":"x"}"#).unwrap();
    assert!(matches!(&r, Record::User { context, .. } if context.is_empty()));
    assert!(serde_json::to_value(&r).unwrap().get("context").is_none());
}
