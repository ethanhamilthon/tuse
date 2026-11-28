mod common;

use tuse_core::{Mode, Options, Session};

#[test]
fn failed_reload_keeps_registered_code_and_config() {
    let (dir, cfg) = common::setup("validation", "localhost:1", "");
    unsafe { std::env::set_var("TUSE_GLOBAL_CONFIG", &cfg) };
    std::fs::write(
        &cfg,
        "return { work = { prompts = { ping = 'before' }, commands = { ping = function() end } } }",
    )
    .unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        mode: Mode::Json,
        resume: None,
        start: Default::default(),
        flags: Default::default(),
    })
    .unwrap();
    for (body, key) in [
        ("return function() return {} end", "expected a table"),
        ("return { history = { hide = {'usage'} } }", "history"),
        ("return { plugins = {} }", "plugins"),
        (
            "return { work = { prompts = { ping = function() end } } }",
            "expected prompt text",
        ),
        (
            "return { work = { commands = { ping = { run = function() end } } } }",
            "expected a function",
        ),
        ("return { open = {} }", "open"),
        ("return { models = { { name = 'p/m' } } }", "models"),
        (
            "return { fs = { functions = { read = function() end } } }",
            "reserved namespace",
        ),
        (
            "return { work = { hook = { lua_end = function() end } } }",
            "unknown event",
        ),
    ] {
        std::fs::write(&cfg, body).unwrap();
        let err = s.reload().unwrap_err();
        assert!(err.contains(key), "{err}");
        assert!(s.config().prompts.contains_key("work:ping"));
    }
    assert!(s.run_command("work:ping", &["unused".into()]).is_err());
    s.close();
    common::cleanup(&dir);
}
