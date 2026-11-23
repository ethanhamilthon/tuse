mod common;

use std::fs;
use tuse_core::{Mode, Options, Session, Start};

#[test]
fn validates_user_config_plugins_and_preserves_state_on_failed_reload() {
    let addr = common::provider(vec![]);
    let (dir, cfg) = common::setup("validation", &addr, "");
    let open = || {
        Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
        })
    };

    let bad_cases = [
        ("return { include = { 'x.lua' } }", "include: was removed"),
        ("return { model = 'p/m' }", "model: was removed"),
        ("return { provider = 'p' }", "provider: was removed"),
        ("return { effort = 'high' }", "effort: was removed"),
        (
            "return { models = { { name = 'm' } } }",
            "models[1].name: expected a provider/model name",
        ),
        (
            "return { models = { { name = 'p/m', windo = 1 } } }",
            "models[1].windo: unknown key",
        ),
        (
            "return { models = { { name = 'p/m', prices = { input = -1 } } } }",
            "models[1].prices.input: expected a finite non-negative number",
        ),
        ("return { unknown_top = 1 }", "unknown_top: unknown key"),
        (
            "return { settings = { nope = 1 } }",
            "settings.nope: unknown key",
        ),
        (
            "return { settings = { retries = 'x' } }",
            "settings.retries: expected integer",
        ),
        (
            "return { settings = { ui = { scroll_step = 0 } } }",
            "settings.ui.scroll_step: value 0 out of range",
        ),
        (
            "return { settings = { ui = { input_max_height = 0 } } }",
            "settings.ui.input_max_height: must be -1 or >= 1",
        ),
        (
            "return { theme = { accent = 'notacolor' } }",
            "theme.accent: invalid color",
        ),
        (
            "return { theme = { bogus = 'red' } }",
            "theme.bogus: unknown color token",
        ),
        ("return { systems = { a = 'x' } }", "systems: expected list"),
        (
            "return { hooks = { h = { events = {'process_start'}, run = function() end } } }",
            "hooks.h.events[1]: event `process_start` was renamed to `start`",
        ),
        (
            "return { hooks = { h = { events = {'process_end'}, run = function() end } } }",
            "hooks.h.events[1]: event `process_end` was renamed to `close`",
        ),
        (
            "return { hooks = { h = { events = {'bogus'}, run = function() end } } }",
            "hooks.h.events[1]: unknown event `bogus`",
        ),
        (
            "return { hooks = { h = { events = {}, run = function() end } } }",
            "hooks.h.events: must contain at least one event",
        ),
        (
            "return { hooks = { h = { events = {'start'} } } }",
            "hooks.h.run: is required",
        ),
        (
            "return { functions = { f = { description = 'ok', modes = {'tui'}, run = function() end } } }",
            "functions.f.modes: was removed",
        ),
        (
            "return { commands = { session = function() end } }",
            "commands.session: cannot override built-in",
        ),
        (
            "return { commands = { bad = 1 } }",
            "commands.bad: expected function",
        ),
        (
            "return { open = { ['.md'] = function() end } }",
            "open..md: extension must not start with `.`",
        ),
        (
            "return { open = { md = 'nope' } }",
            "open.md: expected function",
        ),
    ];
    for (src, want) in bad_cases {
        fs::write(&cfg, src).unwrap();
        let err = open().err().unwrap();
        assert!(
            err.contains("tuse.lua") && err.contains(want),
            "src={src}\nerr={err}"
        );
    }

    // Plugin conflicts & invalid plugin config fail reload without disturbing active config/callbacks.
    let p1 = dir.join("p1");
    let p2 = dir.join("p2");
    fs::create_dir_all(&p1).unwrap();
    fs::create_dir_all(&p2).unwrap();
    fs::write(
        p1.join("init.lua"),
        "return { open = { md = function() end } }",
    )
    .unwrap();
    fs::write(
        p2.join("init.lua"),
        "return { open = { md = function() end } }",
    )
    .unwrap();
    fs::write(
        &cfg,
        r#"return {
            prompts = { keep = "alive" },
            commands = { ok = function() require("tuse").show_markdown("m", "still-active", false) end },
        }"#,
    )
    .unwrap();
    let s = open().unwrap();
    fs::write(
        &cfg,
        r#"return { plugins = { { "a", dir = "p1" }, { "b", dir = "p2" } } }"#,
    )
    .unwrap();
    let err = s.reload().unwrap_err();
    assert!(err.contains("both define open.md"), "{err}");
    assert_eq!(s.config().prompts["keep"], "alive");
    s.run_command("ok").unwrap();
    let dl = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < dl {
        if s.ui_snapshot()
            .markdowns
            .iter()
            .any(|m| m.text == "still-active")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        s.ui_snapshot()
            .markdowns
            .iter()
            .any(|m| m.text == "still-active")
    );
    s.close();
    common::cleanup(&dir);
}
