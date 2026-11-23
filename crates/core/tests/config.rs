mod common;

use std::fs;
use tuse_core::{Event, Mode, Options, Record, Session, Start, UserInput};

#[test]
fn includes_external_configs_and_keeps_require_project_local() {
    let addr = common::provider(vec![
        common::Reply::Call("lua", r#"{"code":"print(shared())"}"#.into()),
        common::Reply::Text("done"),
    ]);
    let (dir, _) = common::setup("config-include", &addr, "");
    let project = dir.join("project");
    let shared = dir.join("shared");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&shared).unwrap();
    let global = shared.join("main.lua");
    // SAFETY: this integration test is the only test in its process, and the mock
    // provider does not read the environment.
    unsafe { std::env::set_var("TUSE_GLOBAL_CONFIG", &global) };
    fs::write(project.join("local_module.lua"), "return 'project module'").unwrap();
    fs::write(shared.join("local_module.lua"), "return 'wrong module'").unwrap();
    fs::write(shared.join("external_only.lua"), "return true").unwrap();
    fs::write(
        shared.join("base.lua"),
        "return {systems = {'first'}, effort = 'low'}",
    )
    .unwrap();
    fs::write(&global, r#"
        assert(not pcall(require, 'external_only'))
        local module = require('local_module')
        assert(os.getenv('TUSE_MODE') == TUSE_MODE)
        assert(sh('printf %s "$TUSE_MODE"').out == TUSE_MODE)
        return {
            include = {'base.lua'}, effort = 'high', systems = {module, TUSE_MODE},
            settings = { retries = 1, ui = { pad_x = 3 } },
            prompts = { local_wins = 'from global', shared = 'kept' },
            functions = { shared = { description = 'A helper', run = function() return module end } },
            hooks = { ignored = { events = {}, run = function() end } },
        }
    "#).unwrap();
    let root = project.join("tuse.lua");
    fs::write(
        &root,
        r#"
        return {
            include = { os.getenv('TUSE_GLOBAL_CONFIG') },
            model = 'p/m', systems = {'last'},
            settings = { ui = { scroll_step = 5 } },
            prompts = { local_wins = 'from project' },
        }
    "#,
    )
    .unwrap();
    let open = |mode| {
        Session::open(Options {
            cwd: project.clone(),
            config: None,
            mode,
            resume: None,
            start: Start::Launch,
        })
    };
    for (mode, name) in [(Mode::Tui, "tui"), (Mode::Json, "json")] {
        let s = open(mode).unwrap();
        let cfg = s.config();
        assert_eq!(cfg.systems, ["first", "project module", name, "last"]);
        assert_eq!(cfg.effort, "high");
        assert_eq!(cfg.settings.retries, 1);
        assert_eq!((cfg.settings.ui.pad_x, cfg.settings.ui.scroll_step), (3, 5));
        assert_eq!(cfg.prompts["local_wins"], "from project");
        assert_eq!(cfg.prompts["shared"], "kept");
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
                |r| matches!(r, Record::Tool {output, ..} if output.trim_end() == "project module")
            ));
        }
        s.close();
    }
    fs::write(shared.join("base.lua"), "return {include = {'./main.lua'}}").unwrap();
    assert!(
        open(Mode::Json)
            .err()
            .unwrap()
            .contains("cyclic config include")
    );
    fs::write(&root, "return {include = {file = 'bad.lua'}}").unwrap();
    assert!(
        open(Mode::Json)
            .err()
            .unwrap()
            .contains("include must be a list")
    );
    common::cleanup(&dir);
}
