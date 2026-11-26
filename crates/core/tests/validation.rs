mod common;

use std::fs;
use tuse_core::{Mode, Options, Session, Start};

#[test]
fn validates_config_and_preserves_state_on_failed_reload() {
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
        ("return { plugins = {} }", "plugins: unknown key"),
        (
            "return { plugins = function() end }",
            "plugins: unknown key",
        ),
        ("return { include = { 'x.lua' } }", "include: unknown key"),
        ("return { model = 'p/m' }", "model: unknown key"),
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
            "settings.retries: unknown key",
        ),
        (
            "return { settings = { max_image_bytes = 1 } }",
            "settings.max_image_bytes: unknown key",
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
            "theme.bogus: unknown key",
        ),
        (
            "return { systems = { a = 'x' } }",
            "systems: expected a list",
        ),
        (
            "return { hooks = { h = { events = {'bogus'}, run = function() end } } }",
            "hooks.h.events: unknown event `bogus`",
        ),
        (
            "return { hooks = { h = { events = {}, run = function() end } } }",
            "hooks.h.events: must contain at least one event",
        ),
        (
            "return { hooks = { h = { events = {'process_start'} } } }",
            "hooks.h.run: is required",
        ),
        (
            "return { functions = { f = { description = 'ok', run = function() end } } }",
            "functions.f: expected function or false, got table",
        ),
        (
            "return { functions = { os = false } }",
            "functions: 'os': reserved name",
        ),
        (
            "return { functions = { package = false } }",
            "functions: 'package': reserved name",
        ),
        (
            "return { functions = { require = function() end } }",
            "functions: 'require': reserved name",
        ),
        (
            "return { functions = { typo = false } }",
            "functions.typo: cannot disable unknown function",
        ),
        (
            "return { functions = { json = function() end } }",
            "functions.json: 'json' is a table and can only be disabled with false",
        ),
        (
            "return { functions = { f = true } }",
            "functions.f: expected function or false, got boolean",
        ),
        (
            "return { commands = { session = function() end } }",
            "commands: `session`: cannot override built-in",
        ),
        (
            "return { commands = { bad = 1 } }",
            "commands.bad: expected function",
        ),
        (
            "return { commands = { c = { desc = 1, run = function() end } } }",
            "commands.c.desc: expected a string",
        ),
        (
            "return { commands = { c = { desc = 'x' } } }",
            "commands.c.run: is required",
        ),
        (
            "return { commands = { c = { args = { 'a b' }, run = function() end } } }",
            "commands.c.args[1]: must not contain whitespace",
        ),
        (
            "return { commands = { c = { nope = 1, run = function() end } } }",
            "commands.c.nope: unknown key",
        ),
        (
            "return { settings = { open = { ['.md'] = function() end } } }",
            "settings.open: `.md`: extension must not start with `.`",
        ),
        (
            "return { settings = { open = { md = 'nope' } } }",
            "settings.open.md: expected function",
        ),
        (
            "return { settings = { open = { ['@file'] = function() end } } }",
            "settings.open: `@file`: unknown special key",
        ),
        (
            "return { open = { md = function() end } }",
            "open: moved to settings.open",
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

    fs::write(
        &cfg,
        r#"return {
            prompts = { keep = "alive" },
            commands = { ok = function() require("tuse").show("m", "still-active", false) end },
        }"#,
    )
    .unwrap();
    let s = open().unwrap();
    fs::write(&cfg, r#"return { plugins = { "tuse/default" } }"#).unwrap();
    let err = s.reload().unwrap_err();
    assert!(err.contains("plugins: unknown key"), "{err}");
    assert_eq!(s.config().prompts["keep"].body, "alive");
    s.run_command("ok", &[]).unwrap();
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

#[test]
fn commands_take_words_by_parameters_or_args() {
    let addr = common::provider(vec![]);
    let extra = r##"
  commands = {
    mv = function(a, b) require("tuse").show("mv", tostring(a) .. "|" .. tostring(b), false) end,
    vary = function(...) require("tuse").show("vary", tostring(select("#", ...)), false) end,
    deploy = {
      desc = "ship it",
      args = { "staging", "prod" },
      run = function(env) require("tuse").show("deploy", tostring(env), false) end,
    },
  },
"##;
    let (dir, cfg) = common::setup("command-words", &addr, extra);
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let words = |w: &[&str]| w.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let shown = |text: &str| {
        let dl = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < dl {
            if s.ui_snapshot().markdowns.iter().any(|m| m.text == text) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    };

    let config = s.config();
    assert_eq!(config.commands["mv"].params, Some(2));
    assert_eq!(config.commands["vary"].params, None);
    assert_eq!(config.commands["deploy"].params, Some(1));
    assert_eq!(config.commands["deploy"].desc, "ship it");
    assert_eq!(config.commands["deploy"].args, ["staging", "prod"]);

    s.run_command("mv", &words(&["a.txt", "b c"])).unwrap();
    assert!(shown("a.txt|b c"));
    s.run_command("mv", &words(&["a.txt"])).unwrap();
    assert!(shown("a.txt|nil"));
    assert_eq!(
        s.run_command("mv", &words(&["a", "b", "c"])).unwrap_err(),
        "/mv takes at most 2 words"
    );
    s.run_command("vary", &words(&["1", "2", "3"])).unwrap();
    assert!(shown("3"));
    s.run_command("deploy", &words(&["prod"])).unwrap();
    assert!(shown("prod"));
    assert_eq!(
        s.run_command("deploy", &words(&["nope"])).unwrap_err(),
        "Unknown arg nope for /deploy"
    );
    s.close();
    common::cleanup(&dir);
}
