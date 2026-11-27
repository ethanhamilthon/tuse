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
        ("return { ui = {} }", "flat top-level section 'ui'"),
        (
            "return { prompts = { a = 'b' } }",
            "flat top-level section 'prompts'",
        ),
        ("return { plugins = false }", "plugins: expected a table"),
        (
            "return { plugins = function() end }",
            "plugins: expected a table",
        ),
        (
            "return { plugins = { only = {}, without = {} } }",
            "plugins: cannot set both only and without",
        ),
        (
            "return { plugins = { only = { 'nope' } } }",
            "plugins.only[1]: unknown plugin `nope`",
        ),
        (
            "return { plugins = { opts = { nope = {} } } }",
            "plugins.opts.nope: unknown plugin `nope`",
        ),
        (
            "return { plugins = { nope = 1 } }",
            "plugins.nope: unknown key",
        ),
        (
            "return { global = 123 }",
            "global: expected false or a table",
        ),
        ("return { top = 123 }", "top: expected a table"),
        (
            "return { ns = { include = { 'x.lua' } } }",
            "ns.include: unknown key",
        ),
        ("return { ns = { model = 'p/m' } }", "ns.model: unknown key"),
        (
            "return { ns = { models = { { name = 'm' } } } }",
            "models[1].name: expected a provider/model name",
        ),
        (
            "return { ns = { models = { { name = 'p/m', windo = 1 } } } }",
            "models[1].windo: unknown key",
        ),
        (
            "return { ns = { models = { { name = 'p/m', prices = { input = -1 } } } } }",
            "models[1].prices.input: expected a finite non-negative number",
        ),
        (
            "return { ns = { unknown_top = 1 } }",
            "ns.unknown_top: unknown key",
        ),
        (
            "return { ns = { settings = { nope = 1 } } }",
            "ns.settings: moved to ui, history, and open",
        ),
        (
            "return { ns = { ui = { nope = 1 } } }",
            "ui.nope: unknown key",
        ),
        (
            "return { ns = { ui = { scroll_step = 3 } } }",
            "ui.scroll_step: unknown key",
        ),
        (
            "return { ns = { ui = { lines = { input = 0 } } } }",
            "ui.lines.input: must be -1 or >= 1",
        ),
        (
            "return { ns = { theme = { accent = 'red' } } }",
            "ns.theme: moved to ui.theme",
        ),
        (
            "return { ns = { ui = { theme = { accent = 'notacolor' } } } }",
            "ui.theme.accent: invalid color",
        ),
        (
            "return { ns = { ui = { theme = { bogus = 'red' } } } }",
            "ui.theme.bogus: unknown key",
        ),
        (
            "return { ns = { ui = { theme = { user = 'input', input = 'user' } } } }",
            "ui.theme.user: link cycle",
        ),
        (
            "return { ns = { systems = { a = 'x' } } }",
            "systems: expected a list",
        ),
        (
            "return { ns = { systems = { function() end } } }",
            "ns.systems[1]: unsupported value type function",
        ),
        (
            "return { ns = { hooks = { h = { events = {'bogus'}, run = function() end } } } }",
            "hooks.h.events: unknown event `bogus`",
        ),
        (
            "return { ns = { hooks = { h = { events = {}, run = function() end } } } }",
            "hooks.h.events: must contain at least one event",
        ),
        (
            "return { ns = { hooks = { h = { events = {'process_start'} } } } }",
            "ns.hooks.h.run: is required",
        ),
        (
            "return { ns = { functions = { f = { description = 'ok', run = function() end } } } }",
            "ns.functions.f: expected function or false, got table",
        ),
        (
            "return { ns = { functions = { os = false } } }",
            "functions: 'os': reserved name",
        ),
        (
            "return { ns = { functions = { package = false } } }",
            "functions: 'package': reserved name",
        ),
        (
            "return { ns = { functions = { require = function() end } } }",
            "functions: 'require': reserved name",
        ),
        (
            "return { ns = { functions = { typo = false } } }",
            "functions.typo: cannot disable unknown function",
        ),
        (
            "return { ns = { functions = { json = function() end } } }",
            "functions: 'json' is a table and can only be disabled with false",
        ),
        (
            "return { ns = { functions = { f = true } } }",
            "ns.functions.f: expected function or false, got boolean",
        ),
        (
            "return { ns = { commands = { session = { run = function() end } } } }",
            "commands: `session`: cannot override built-in",
        ),
        (
            "return { ns = { commands = { bad = 1 } } }",
            "ns.commands.bad: expected a table",
        ),
        (
            "return { ns = { commands = { c = function() end } } }",
            "ns.commands.c: expected a table",
        ),
        (
            "return { ns = { commands = { c = { desc = 1, run = function() end } } } }",
            "ns.commands.c.desc: expected a string",
        ),
        (
            "return { ns = { commands = { c = { desc = 'x' } } } }",
            "ns.commands.c.run: is required",
        ),
        (
            "return { ns = { commands = { c = { args = { 'a b' }, run = function() end } } } }",
            "ns.commands.c.args[1]: must not contain whitespace",
        ),
        (
            "return { ns = { commands = { c = { args = { [1] = 'a', [3] = 'b' }, run = function() end } } } }",
            "ns.commands.c.args: list has hole at index 2",
        ),
        (
            "return { ns = { commands = { c = { nope = 1, run = function() end } } } }",
            "ns.commands.c.nope: unknown key",
        ),
        (
            "return { ns = { open = { ['.md'] = function() end } } }",
            "open: `.md`: extension must not start with `.`",
        ),
        (
            "return { ns = { open = { md = 'nope' } } }",
            "ns.open.md: expected function",
        ),
        (
            "return { ns = { open = { ['@file'] = function() end } } }",
            "open: `@file`: unknown special key",
        ),
        (
            "return { ns = { settings = { open = { md = function() end } } } }",
            "ns.settings: moved to ui, history, and open",
        ),
        (
            "return { a = { prompts = { x = '1' } }, b = { prompts = { x = '2' } } }",
            "prompts.x: duplicate",
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
        r#"return { ns = { providers = { x = { base = 'http://x', key = 'super-secret-value', nope = 1 } } } }"#,
    )
    .unwrap();
    let err = open().err().unwrap();
    assert!(!err.contains("super-secret-value"), "{err}");

    fs::write(
        &cfg,
        r#"return {
            ns = {
                prompts = { keep = "alive" },
                commands = { ok = { run = function() require("tuse").show("m", "still-active", false) end } },
            },
        }"#,
    )
    .unwrap();
    let s = open().unwrap();
    fs::write(&cfg, r#"return { bad = { nope = 1 } }"#).unwrap();
    let err = s.reload().unwrap_err();
    assert!(err.contains("bad.nope: unknown key"), "{err}");
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
    mv = { run = function(a, b) require("tuse").show("mv", tostring(a) .. "|" .. tostring(b), false) end },
    vary = { run = function(...) require("tuse").show("vary", tostring(select("#", ...)), false) end },
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
