mod common;

use std::fs;
use std::sync::Mutex;
use tuse_core::{Event, Mode, Options, Record, Session, Start, UserInput};

static ENV: Mutex<()> = Mutex::new(());

#[test]
fn require_session_validation_and_reload_rollback() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
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
    unsafe {
        for v in [
            "TUSE_MODE",
            "TUSE_SESSION",
            "TUSE_CWD",
            "TUSE_MODEL",
            "TUSE_CURRENT_CONFIG",
            "TUSE_GLOBAL_CONFIG",
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
        assert(s.id == nil and s.model == nil and s.history == nil, 'pre-session snapshot')
        assert(s.mode == 'tui' or s.mode == 'json')
        assert(s.cwd ~= nil and s.home ~= nil and s.config_path ~= nil)
        for name in pairs(t) do assert(_G[name] == nil, name) end
        local sh_out = t.sh('printf "%s|%s|%s" "${TUSE_MODE:-none}" "${TUSE_SESSION:-none}" "$CUSTOM_USER_VAR"').out
        assert(sh_out == 'none|none|kept_val', 'sh env: ' .. sh_out)
        assert(t.read('fixture.txt') == 'from config')
        assert(TUSE_MODE == nil)
        local dir = os.getenv('THIS_PATH'):match('(.*)/[^/]+$')
        assert(require(dir .. '/lib') == 'lib.lua wins')
        assert(require(dir .. '/lib/file.lua') == 'lib file')
        return function()
            return {
                shared = {
                    systems = { 'first', s.mode },
                    ui = { space = { padding = 3 } },
                    prompts = { shared = 'kept', rex = rex.tag },
                    functions = {
                        from_global = function()
                            assert(sh == nil and read == nil)
                            assert(t.session().id ~= nil)
                            return require('rex').tag
                        end,
                    },
                    commands = { ping = { run = function() require('tuse').show('p', require('rex').tag, false) end } },
                },
            }
        end
    "#,
    )
    .unwrap();

    let root = project.join("tuse.lua");
    fs::write(
        &root,
        r#"
        assert(sh == nil and read == nil and _G.json == nil)
        assert(load("return sh")() == nil)
        assert(type(require('global')) == 'function')
        assert(require('util').origin == 'project' and require('pkg') == 'pkg init')
        return {
            project = {
                providers = { project = {
                    base = 'http://127.0.0.1:1', key = 'local-secret',
                    headers = { ['X-Tenant'] = '$CUSTOM_USER_VAR' },
                } },
                systems = { 'last' },
                ui = { space = { steps = 5 } },
                prompts = { local_wins = 'from project' },
            },
        }
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
        let profile = cfg.connect("project").unwrap();
        assert_eq!(profile.api, "openai-chat");
        assert_eq!(profile.headers["x-tenant"], "kept_val");
        assert!(s.model_available("project/m"));
        assert!(!tuse_core::Providers::json().unwrap().contains("project"));
        assert!(
            !serde_json::to_string(&*cfg)
                .unwrap()
                .contains("local-secret")
        );
        assert_eq!((cfg.ui.space.padding, cfg.ui.space.steps), (3, 5));
        assert_eq!(cfg.prompts["local_wins"].body, "from project");
        assert_eq!(cfg.prompts["rex"].body, "rex-v1");
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
                |r| matches!(r, Record::Lua { output, .. } if output.trim_end() == "project:rex-v1")
            ));
            fs::write(shared.join("rex.lua"), "return { tag = 'rex-v2' }").unwrap();
            s.reload().unwrap();
            assert_eq!(s.config().prompts["rex"].body, "rex-v2");
            let saved_root = fs::read_to_string(&root).unwrap();
            fs::write(&root, "return { bad = { unknown_typo = 123 } }").unwrap();
            let err = s.reload().unwrap_err();
            assert!(
                err.contains("tuse.lua") && err.contains("bad.unknown_typo: unknown key"),
                "{err}"
            );
            assert_eq!(s.config().prompts["rex"].body, "rex-v2");
            s.run_command("ping", &[]).unwrap();
            let dl = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::time::Instant::now() < dl {
                if s.ui_snapshot().markdowns.iter().any(|m| m.text == "rex-v2") {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            assert!(s.ui_snapshot().markdowns.iter().any(|m| m.text == "rex-v2"));
            fs::write(
                &root,
                "return { local_p = { providers = { p = { base = 'http://127.0.0.1:1', key = 'k' } } } }",
            )
            .unwrap();
            assert!(s.reload().unwrap_err().contains("name is already saved"));
            assert!(s.model_available("project/m"));
            fs::write(&root, "return { local_p = { providers = {} } }").unwrap();
            s.reload().unwrap();
            assert!(!s.model_available("project/m"));
            assert!(s.model_available("p/m"));
            fs::write(&root, saved_root).unwrap();
            fs::write(shared.join("rex.lua"), "return { tag = 'rex-v1' }").unwrap();
        }
        s.close();
    }

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

    let globals = tuse_config::module_candidates("global", std::path::Path::new("."));
    assert_eq!(globals.len(), 2);
    assert!(globals[0].ends_with(".config/tuse.lua"));
    assert!(globals[1].ends_with(".config/tuse/init.lua"));
    common::cleanup(&dir);
}

#[test]
fn global_options_plugins_selectors_and_preload_reload() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let addr = common::provider(vec![]);
    let (dir, _) = common::setup("config-plugins", &addr, "");
    let project = dir.join("project");
    let shared = dir.join("home/.config/tuse");
    let plugins = dir.join("home/.tuse/plugins");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&shared).unwrap();
    fs::create_dir_all(plugins.join("alpha")).unwrap();
    fs::create_dir_all(plugins.join("beta")).unwrap();

    fs::write(
        plugins.join("alpha/tuse-plugin.json"),
        r#"{"name":"alpha","version":"1.2.3"}"#,
    )
    .unwrap();
    fs::write(
        plugins.join("alpha/init.lua"),
        r#"return function(opts)
            local tag = (opts and opts.tag) or 'default'
            return {
                systems = { 'alpha:' .. tag },
                prompts = { alpha = tag },
                ui = { space = { padding = 1 } },
            }
        end"#,
    )
    .unwrap();
    fs::write(
        plugins.join("beta/tuse-plugin.json"),
        r#"{"name":"beta","version":"0.1.0"}"#,
    )
    .unwrap();
    fs::write(
        plugins.join("beta/init.lua"),
        r#"return { systems = { 'beta' }, prompts = { beta = 'b' } }"#,
    )
    .unwrap();

    let custom_global = dir.join("custom-global");
    fs::create_dir_all(&custom_global).unwrap();
    let flag_file = dir.join("global-ran");
    fs::write(
        custom_global.join("init.lua"),
        format!(
            r#"return function(opts)
                local f = assert(io.open({:?}, 'w'))
                f:write('ran')
                f:close()
                return {{
                    base = {{
                        systems = {{ 'global:' .. ((opts and opts.env) or 'none') }},
                        ui = {{ space = {{ padding = 2, steps = 4 }} }},
                    }},
                }}
            end"#,
            flag_file.to_string_lossy()
        ),
    )
    .unwrap();

    unsafe {
        std::env::set_var("TUSE_GLOBAL_CONFIG", &custom_global);
    }

    let root = project.join("tuse.lua");
    fs::write(
        &root,
        r#"
        package.preload['cached_mod'] = function() return 'v1' end
        assert(require('cached_mod') == 'v1')
        return {
            global = { env = 'prod' },
            plugins = { without = { 'beta' }, opts = { alpha = { tag = 'custom' } } },
            local_ns = {
                systems = { 'local' },
                ui = { space = { padding = 3 } },
            },
        }
    "#,
    )
    .unwrap();

    let open = || {
        Session::open(Options {
            cwd: project.clone(),
            config: None,
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
        })
    };

    let s = open().unwrap();
    let cfg = s.config();
    assert_eq!(cfg.systems, ["global:prod", "alpha:custom", "local"]);
    assert_eq!(cfg.prompts["alpha"].body, "custom");
    assert!(!cfg.prompts.contains_key("beta"));
    assert_eq!((cfg.ui.space.padding, cfg.ui.space.steps), (3, 4));
    assert_eq!(cfg.plugins.len(), 1);
    assert_eq!(cfg.plugins[0].name, "alpha");
    assert_eq!(cfg.plugins[0].version, "1.2.3");
    assert!(flag_file.exists());

    fs::write(
        &root,
        r#"
        assert(package.preload['cached_mod'] == nil)
        return {
            plugins = { only = { 'alpha' } },
            dup = require('plugin:alpha')(),
        }
    "#,
    )
    .unwrap();
    let err = s.reload().unwrap_err();
    assert!(err.contains("duplicate"), "{err}");

    fs::remove_file(&flag_file).unwrap();
    fs::write(
        &root,
        r#"
        local all = require('plugins')
        assert(all.prompts.alpha == 'default' and all.prompts.beta == 'b')
        return {
            global = false,
            plugins = { only = {} },
            pick = { prompts = { beta = all.prompts.beta }, systems = { 'beta' } },
        }
    "#,
    )
    .unwrap();
    s.reload().unwrap();
    assert!(!flag_file.exists());
    assert_eq!(s.config().systems, ["beta"]);
    assert!(s.config().plugins.is_empty());
    s.close();

    fs::write(&root, "return {}").unwrap();
    unsafe {
        std::env::set_var("TUSE_GLOBAL_CONFIG", dir.join("missing.lua"));
    }
    assert!(open().is_err());
    unsafe {
        std::env::remove_var("TUSE_GLOBAL_CONFIG");
    }
    common::cleanup(&dir);
}
#[test]
fn scoped_reload_merges_only_the_active_namespace_and_rolls_back() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let (dir, root) = common::setup("scoped-reload", "localhost:1", "");
    let global = dir.join("home/.config/tuse.lua");
    fs::create_dir_all(global.parent().unwrap()).unwrap();
    let write_global = |value| {
        fs::write(
            &global,
            format!(
                r#"
            return function()
                global_runs = (global_runs or 0) + 1
                return {{work = {{systems = {{'{value}:' .. global_runs}}}}}}
            end
        "#
            ),
        )
        .unwrap();
    };
    let write_local = |module, extra: &str, other: &str| {
        fs::write(
            &root,
            format!(
                r#"
            local_runs = (local_runs or 0) + 1
            package.preload.helper = function() return '{module}' end
            local helper = require('helper')
            local t = require('tuse')
            return {{
                other = {{systems = {{'{other}'}}, commands = {{ping = {{run = function() end}}}}}},
                added = {{systems = {{'added'}}}},
                work = {{
                    systems = {{helper .. ':' .. local_runs}},
                    commands = {{check = {{run = function()
                        t.show('checked', helper .. ':' .. require('helper'), false)
                    end}}, {extra}}},
                }},
            }}
        "#
            ),
        )
        .unwrap();
    };
    write_global("global-old");
    write_local("old", "", "other-old");
    let initial = fs::read_to_string(&root)
        .unwrap()
        .replace("added = {systems = {'added'}},", "");
    fs::write(&root, initial).unwrap();
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(root.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let reload = |name: &str| session.reload_namespace(Some(name.into()));
    tuse_core::Providers::remove("p").unwrap();
    write_global("global-new");
    write_local("new", "", "other-new");
    assert!(
        reload("added")
            .unwrap_err()
            .contains("unknown active namespace")
    );
    reload("work").unwrap();
    let expected = ["global-new:2", "other-old", "new:2"];
    assert_eq!(session.config().systems, expected);
    assert_eq!(session.config().connect[0].name, "p");
    assert!(
        session.history()[0].fields["text"]
            .as_str()
            .unwrap()
            .contains("new:2")
    );

    write_local("broken", "ping = {run = function() end}", "other-new");
    assert!(reload("work").unwrap_err().contains("duplicate"));
    fs::write(&root, "return {}").unwrap();
    assert!(reload("work").unwrap_err().contains("missing"));
    assert_eq!(session.config().systems, expected);
    session.run_command("check", &[]).unwrap();
    for _ in 0..100 {
        if session
            .ui_snapshot()
            .markdowns
            .iter()
            .any(|m| m.name == "checked")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(session.ui_snapshot().markdowns[0].text, "new:new");
    session.close();
    common::cleanup(&dir);
}
