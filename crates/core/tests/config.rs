mod common;

use std::fs;
use tuse_core::{Event, Mode, Options, Record, Session, Start, UserInput};

#[test]
fn require_session_validation_and_reload_rollback() {
    let addr = common::provider(vec![
        common::Reply::Call(
            "lua",
            r#"{"code":"assert(sh == require('tuse').sh and type(read) == 'function' and require('tuse').env == nil); print(require('util').origin .. ':' .. from_global())"}"#.into(),
        ),
        common::Reply::Text("done"),
    ]);
    let (dir, _) = common::setup("config-require", &addr, "");
    let project = dir.join("project");
    let shared = dir.join("home/.config/tuse");
    fs::create_dir_all(project.join("pkg")).unwrap();
    fs::create_dir_all(shared.join("sub")).unwrap();
    fs::create_dir_all(shared.join("lib")).unwrap();
    let global = shared.join("init.lua");
    // SAFETY: single test in this binary; clear any inherited vars from an outer Tuse process.
    unsafe {
        for v in [
            "TUSE_MODE",
            "TUSE_SESSION",
            "TUSE_CWD",
            "TUSE_MODEL",
            "TUSE_CURRENT_CONFIG",
        ] {
            std::env::remove_var(v);
        }
        std::env::set_var("CUSTOM_USER_VAR", "kept_val");
    }

    fs::write(project.join("fixture.txt"), "from config").unwrap();
    fs::write(project.join("util.lua"), "return { origin = 'project' }").unwrap();
    fs::write(project.join("pkg/init.lua"), "return 'pkg init'").unwrap();
    fs::write(shared.join("util.lua"), "return { origin = 'shared' }").unwrap();
    fs::write(shared.join("rex.lua"), "return { tag = 'rex-v1' }").unwrap();
    fs::write(
        shared.join("sub/leaf.lua"),
        "assert(sh == nil and _G.read == nil); return 'leaf'",
    )
    .unwrap();
    fs::write(
        shared.join("sub/mod.lua"),
        "return { leaf = require('leaf'), rex = require('../rex') }",
    )
    .unwrap();
    fs::write(shared.join("lib/init.lua"), "return 'lib init'").unwrap();
    fs::write(shared.join("lib/file.lua"), "return 'lib file'").unwrap();
    fs::write(shared.join("lib.lua"), "return 'lib.lua wins'").unwrap();

    fs::write(
        &global,
        r#"
        local t = require('tuse')
        local rex = require('rex')
        local util = require('util')
        local sub = require('sub.mod')
        assert(util.origin == 'shared' and sub.leaf == 'leaf' and sub.rex == rex)
        assert(t.env == nil)
        assert(os.getenv('TUSE_MODE') == nil and os.getenv('TUSE_SESSION') == nil)
        assert(os.getenv('CUSTOM_USER_VAR') == 'kept_val')
        local s = t.session()
        assert(s.id == nil and s.model == nil and s.trace == nil, 'pre-session snapshot')
        assert(s.mode == 'tui' or s.mode == 'json')
        assert(s.cwd ~= nil and s.home ~= nil and s.configpath ~= nil)
        for name in pairs(t) do assert(_G[name] == nil, name) end
        local sh_out = t.sh('printf "%s|%s|%s" "${TUSE_MODE:-none}" "${TUSE_SESSION:-none}" "$CUSTOM_USER_VAR"').out
        assert(sh_out == 'none|none|kept_val', 'sh env: ' .. sh_out)
        assert(t.read('fixture.txt') == 'from config')
        assert(TUSE_MODE == nil)
        local dir = os.getenv('THIS_PATH'):match('(.*)/[^/]+$')
        assert(require(dir .. '/lib') == 'lib.lua wins')
        assert(require(dir .. '/lib/file.lua') == 'lib file')
        return {
            systems = { 'first', s.mode },
            settings = { retries = 1, ui = { pad_x = 3 } },
            prompts = { shared = 'kept', rex = rex.tag },
            functions = {
                from_global = {
                    description = 'Reads sibling module at runtime',
                    run = function()
                        assert(sh == nil and read == nil)
                        assert(t.session().id ~= nil)
                        return require('rex').tag
                    end,
                },
            },
            commands = { ping = function() require('tuse').show('p', require('rex').tag, false) end },
        }
    "#,
    )
    .unwrap();

    let root = project.join("tuse.lua");
    fs::write(
        &root,
        r#"
        assert(sh == nil and read == nil and _G.json == nil)
        assert(load("return sh")() == nil)
        local g = require('global')
        assert(require('util').origin == 'project' and require('pkg') == 'pkg init')
        g.systems[#g.systems + 1] = 'last'
        g.settings.ui.scroll_step = 5
        g.prompts.local_wins = 'from project'
        return g
    "#,
    )
    .unwrap();

    let open = |mode, config| {
        Session::open(Options {
            cwd: project.clone(),
            config,
            mode,
            resume: None,
            start: Start::Launch,
        })
    };

    for (mode, name) in [(Mode::Tui, "tui"), (Mode::Json, "json")] {
        let s = open(mode, None).unwrap();
        let cfg = s.config();
        assert_eq!(cfg.systems, ["first", name, "last"]);
        assert_eq!(cfg.settings.retries, 1);
        assert_eq!((cfg.settings.ui.pad_x, cfg.settings.ui.scroll_step), (3, 5));
        assert_eq!(cfg.prompts["local_wins"], "from project");
        assert_eq!(cfg.prompts["rex"], "rex-v1");
        if mode == Mode::Tui {
            let events = s.subscribe();
            s.submit(UserInput::text("test"));
            while !matches!(
                events
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap(),
                Event::TurnEnd { .. }
            ) {}
            assert!(s.records().iter().any(
                |r| matches!(r, Record::Tool { output, .. } if output.trim_end() == "project:rex-v1")
            ));
            // Reload succeeds and sees the same pre-session semantics (`s.id == nil`).
            fs::write(shared.join("rex.lua"), "return { tag = 'rex-v2' }").unwrap();
            s.reload().unwrap();
            assert_eq!(s.config().prompts["rex"], "rex-v2");
            // Failed reload keeps previous working config, modules, and callbacks active!
            let saved_root = fs::read_to_string(&root).unwrap();
            fs::write(&root, "return { unknown_typo = 123 }").unwrap();
            let err = s.reload().unwrap_err();
            assert!(
                err.contains("tuse.lua") && err.contains("unknown_typo: unknown key"),
                "{err}"
            );
            assert_eq!(s.config().prompts["rex"], "rex-v2");
            s.run_command("ping").unwrap();
            let dl = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::time::Instant::now() < dl {
                if s.ui_snapshot().markdowns.iter().any(|m| m.text == "rex-v2") {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            assert!(s.ui_snapshot().markdowns.iter().any(|m| m.text == "rex-v2"));
            fs::write(&root, saved_root).unwrap();
            fs::write(shared.join("rex.lua"), "return { tag = 'rex-v1' }").unwrap();
        }
        s.close();
    }

    // Cyclic require checks.
    fs::write(project.join("a.lua"), "return require('b')").unwrap();
    fs::write(project.join("b.lua"), "return require('a')").unwrap();
    fs::write(&root, "return require('a')").unwrap();
    assert!(
        open(Mode::Json, None)
            .err()
            .unwrap()
            .contains("cyclic require")
    );
    fs::write(&global, "return require('global')").unwrap();
    assert!(
        open(Mode::Json, Some(global))
            .err()
            .unwrap()
            .contains("cyclic require")
    );

    let globals = tuse_config::global_candidates();
    assert_eq!(globals.len(), 2);
    assert!(globals[0].ends_with(".config/tuse.lua"));
    assert!(globals[1].ends_with(".config/tuse/init.lua"));
    common::cleanup(&dir);
}
