mod common;

use std::fs;
use tuse_core::{Mode, Options, Session, Start};

fn open(dir: &std::path::Path, cfg: &std::path::Path) -> Result<std::sync::Arc<Session>, String> {
    Session::open(Options {
        cwd: dir.to_path_buf(),
        config: Some(cfg.to_path_buf()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
}

#[test]
fn plugins_load_with_opts_and_merge_under_the_user() {
    let addr = common::provider(vec![common::Reply::Text("ok")]);
    let (dir, cfg) = common::setup("plugins", &addr, "");
    let clones = dir.join("home/plugins");
    let repo = clones.join("me/kit");
    fs::create_dir_all(repo.join("plugins/greet")).unwrap();
    fs::write(
        repo.join("init.lua"),
        "return { prompts = { root = 'root' }, systems = { 'from root' } }",
    )
    .unwrap();
    fs::write(repo.join("plugins/greet/util.lua"), "return 'Hi '").unwrap();
    fs::write(
        repo.join("plugins/greet/init.lua"),
        r#"assert(os.getenv('THIS_PATH'):match('plugins/greet/init.lua$'))
        return function(opts)
            return {
                systems = { require('./util') .. (opts.name or 'friend') },
                prompts = { greet = 'g', shared = 'plugin' },
                settings = { ui = { pad_x = 4 } },
                commands = { hi = function() end },
            }
        end"#,
    )
    .unwrap();
    let local = dir.join("local");
    fs::create_dir_all(&local).unwrap();
    fs::write(
        local.join("tuse.lua"),
        "return { prompts = { dev = 'dev' } }",
    )
    .unwrap();
    let write = |plugins: &str, extra: &str| {
        fs::write(
            &cfg,
            format!(
                "assert(os.getenv('THIS_PATH') == '{}')\nreturn {{ model = 'p/m', plugins = {{ {plugins} }}, {extra} }}",
                cfg.canonicalize().unwrap_or(cfg.clone()).display()
            ),
        )
        .unwrap()
    };
    write(
        r#""me/kit", { "me/kit/plugins/greet", opts = { name = "Yen" } }, { "dev", dir = "local" }, { "me/off", enabled = false }"#,
        "systems = { 'user' }, prompts = { shared = 'user' }, settings = { ui = { scroll_step = 2 } }",
    );
    let s = open(&dir, &cfg).unwrap();
    let c = s.config();
    assert_eq!(c.systems, ["from root", "Hi Yen", "user"]);
    assert_eq!(c.prompts["shared"], "user");
    assert_eq!(
        (c.prompts["greet"].as_str(), c.prompts["dev"].as_str()),
        ("g", "dev")
    );
    assert_eq!((c.settings.ui.pad_x, c.settings.ui.scroll_step), (4, 2));
    assert_eq!(c.commands, ["hi"]);
    drop(s);

    let fails = |plugins: &str, want: &str| {
        write(plugins, "");
        let err = open(&dir, &cfg).err().unwrap();
        assert!(err.contains(want), "{err}");
    };
    fails(r#""me/missing""#, "plugin me/missing: not installed");
    fails(r#"{ "me/kit", opt = {} }"#, "unknown key");
    fails(
        r#"{ "dev", dir = "local" }, { "dev2", dir = "local" }"#,
        "both define prompts.dev",
    );
    fs::write(local.join("tuse.lua"), "return { model = 'x' }").unwrap();
    fails(r#"{ "dev", dir = "local" }"#, "`model` is not allowed");
    fails(
        r#"{ "me/kit", ref = "a" }, { "me/kit/plugins/greet", ref = "b" }"#,
        "conflicting ref",
    );
    common::cleanup(&dir);
}
