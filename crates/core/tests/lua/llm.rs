#[path = "../common/mod.rs"]
mod common;
use common::Reply;
use std::time::Duration;
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

#[test]
fn llm_answers_without_tools_and_is_priced_apart() {
    let code = "local a = llm('say hi', 'p/cheap'); local b = llm('no eff', 'p/noreason'); local ok1, e1 = pcall(llm, 'x', 'p/highonly'); local ok2, e2 = pcall(llm, 'x', 'p/m', 'huge'); return table.concat({a, b, tostring(ok1), e1, tostring(ok2), e2}, '|')";
    let addr = common::provider(vec![
        Reply::Call("lua", serde_json::json!({ "code": code }).to_string()),
        Reply::Text("hi there"),
        Reply::Text("plain"),
        Reply::Text("done"),
    ]);
    let (dir, cfg) = common::setup("lua-llm", &addr, "");
    std::fs::write(&cfg, "return { models = { ['p/m'] = {prices={input=1,output=1}}, ['p/cheap'] = {prices={input=100000,output=100000}}, ['p/noreason'] = {efforts={}}, ['p/highonly'] = {efforts={'high'}} } }").unwrap();
    let s = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg);
        }
        Options {
            cwd: dir.clone(),
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
            flags: Default::default(),
        }
    })
    .unwrap();
    let rx = s.subscribe();
    s.submit(UserInput::text("go"));
    let mut output = String::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::ToolEnd { output: o, .. } => output = o,
            Event::TurnEnd { .. } => break,
            _ => {}
        }
    }
    assert!(
        output.starts_with("hi there|plain|false|")
            && output.contains("p/highonly")
            && output.contains("huge"),
        "{output}"
    );
    let st = s.stats();
    assert_eq!((st.requests, st.llm_requests), (2, 2));
    assert!((st.llm_cost - 1.3).abs() < 1e-9, "{}", st.llm_cost);
    assert!(
        (st.cost - st.llm_cost - 2.0 * 13e-6).abs() < 1e-9,
        "{}",
        st.cost
    );
    let snap = s.snapshot(false);
    let u = snap.usage.unwrap();
    assert_eq!((u.cost, u.llm_cost), (Some(st.cost), Some(st.llm_cost)));
    s.close();
    common::cleanup(&dir);
}
