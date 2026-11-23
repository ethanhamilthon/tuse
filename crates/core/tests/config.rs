mod common;

use std::fs;
use tuse_core::{Event, Mode, Options, Record, Session, Start, UserInput};

#[test]
fn file_relative_require_replaces_include() {
    let addr = common::provider(vec![
        common::Reply::Call(
            "lua",
            r#"{"code":"assert(sh == require('tuse').sh and type(read) == 'function'); print(require('util').origin .. ':' .. from_global())"}"#.into(),
        ),
        common::Reply::Text("done"),
    ]);
    let (dir, _) = common::setup("config-require", &addr, "");
    let project = dir.join("project");
    let shared = dir.join("shared");
    fs::create_dir_all(project.join("pkg")).unwrap();
    fs::create_dir_all(shared.join("sub")).unwrap();
    fs::create_dir_all(shared.join("lib")).unwrap();
    let global = shared.join("init.lua");
    // SAFETY: this integration test is the only test in its process, and the mock
    // provider does not read the environment.
    unsafe { std::env::set_var("TUSE_GLOBAL_CONFIG", &global) };

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
        assert(util.origin == 'shared')
        assert(sub.leaf == 'leaf')
        assert(sub.rex == rex, 'same canonical file shares cache')
        local mode = os.getenv('TUSE_MODE')
        assert(t.env.TUSE_MODE == mode and t.env.TUSE_CWD ~= nil)
        for name in pairs(t) do assert(_G[name] == nil, name) end
        assert(t.sh('printf %s "$TUSE_MODE"').out == mode)
        assert(t.read('fixture.txt') == 'from config')
        assert(t.env.TUSE_GLOBAL_CONFIG == os.getenv('TUSE_GLOBAL_CONFIG'))
        assert(not pcall(function() return TUSE_MODE end))
        local dir = t.env.TUSE_GLOBAL_CONFIG:match('(.*)/[^/]+$')
        assert(require(dir .. '/lib') == 'lib.lua wins')
        assert(require(dir .. '/lib/file.lua') == 'lib file')
        return {
            effort = 'high',
            systems = { 'first', mode },
            settings = { retries = 1, ui = { pad_x = 3 } },
            prompts = { shared = 'kept', rex = rex.tag },
            functions = {
                from_global = {
                    description = 'Reads sibling module at runtime',
                    run = function()
                        assert(sh == nil and read == nil)
                        assert(t.read('fixture.txt') == 'from config')
                        return require('rex').tag
                    end,
                },
            },
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
        assert(require('util').origin == 'project')
        assert(require('pkg') == 'pkg init')
        g.model = 'p/m'
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
        assert_eq!(cfg.effort, "high");
        assert_eq!(cfg.settings.retries, 1);
        assert_eq!((cfg.settings.ui.pad_x, cfg.settings.ui.scroll_step), (3, 5));
        assert_eq!(cfg.prompts["local_wins"], "from project");
        assert_eq!(cfg.prompts["shared"], "kept");
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
            fs::write(shared.join("rex.lua"), "return { tag = 'rex-v2' }").unwrap();
            s.reload().unwrap();
            assert_eq!(s.config().prompts["rex"], "rex-v2");
            fs::write(shared.join("rex.lua"), "return { tag = 'rex-v1' }").unwrap();
        }
        s.close();
    }

    // `include` was removed.
    fs::write(&root, "return { include = { 'util.lua' } }").unwrap();
    assert!(
        open(Mode::Json, None)
            .err()
            .unwrap()
            .contains("`include` was removed")
    );

    // Cyclic require between two modules.
    fs::write(project.join("a.lua"), "return require('b')").unwrap();
    fs::write(project.join("b.lua"), "return require('a')").unwrap();
    fs::write(&root, "return require('a')").unwrap();
    assert!(
        open(Mode::Json, None)
            .err()
            .unwrap()
            .contains("cyclic require")
    );

    // Root config requiring itself via `global`.
    fs::write(&global, "return require('global')").unwrap();
    assert!(
        open(Mode::Json, Some(global.clone()))
            .err()
            .unwrap()
            .contains("cyclic require")
    );

    // Global config lookup uses `tuse.lua` / `tuse/init.lua`, never `main.lua`.
    unsafe { std::env::remove_var("TUSE_GLOBAL_CONFIG") };
    let globals = tuse_config::global_candidates();
    assert_eq!(globals.len(), 2);
    assert!(globals[0].ends_with(".config/tuse.lua"));
    assert!(globals[1].ends_with(".config/tuse/init.lua"));

    common::cleanup(&dir);
}
