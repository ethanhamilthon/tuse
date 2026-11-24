#[path = "../common/mod.rs"]
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
    let clones = dir.join("home/.tuse/plugins");
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
                open = { md = function() end },
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
                "assert(os.getenv('THIS_PATH') == '{}')\nreturn {{ plugins = {{ {plugins} }}, {extra} }}",
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
    assert_eq!(c.open, ["md"]);
    let names: Vec<_> = c
        .plugins
        .iter()
        .map(|p| (p.name.as_str(), p.commit.is_none()))
        .collect();
    assert_eq!(
        names,
        [
            ("me/kit", true),
            ("me/kit/plugins/greet", true),
            ("dev", true)
        ]
    );
    drop(s);

    let fails = |plugins: &str, want: &str| {
        write(plugins, "");
        let err = open(&dir, &cfg).err().unwrap();
        assert!(err.contains(want), "{err}");
    };
    fails(
        r#""me/missing""#,
        "plugin me/missing: not checked out; run `git clone https://github.com/me/missing.git",
    );
    fails(r#"{ "me/kit", opt = {} }"#, "plugins[1].opt: unknown key");
    fails(
        r#"{ "dev", dir = "local" }, { "dev2", dir = "local" }"#,
        "both define prompts.dev",
    );
    fs::write(local.join("tuse.lua"), "return { typo_key = 1 }").unwrap();
    fails(
        r#"{ "dev", dir = "local" }"#,
        "plugin dev: typo_key: unknown key",
    );
    fs::write(local.join("tuse.lua"), "return { plugins = {} }").unwrap();
    fails(
        r#"{ "dev", dir = "local" }"#,
        "plugin dev: plugins: is not allowed",
    );
    fails(
        r#"{ "me/kit", ref = "a" }, { "me/kit/plugins/greet", ref = "b" }"#,
        "conflicting ref",
    );
    let welcome =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/welcome");
    fs::write(
        &cfg,
        format!(
            "return {{ plugins = {{ {{ 'welcome', dir = {:?} }} }} }}",
            welcome.display().to_string()
        ),
    )
    .unwrap();
    let j = open(&dir, &cfg).unwrap();
    assert!(j.config().hooks.is_empty(), "welcome skips json mode");
    j.close();
    let t = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Tui,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    assert!(t.config().hooks.contains_key("welcome"));
    let dl = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < dl {
        if t.ui_snapshot()
            .markdowns
            .iter()
            .any(|m| m.name == "welcome")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let snap = t.ui_snapshot();
    let body = &snap
        .markdowns
        .iter()
        .find(|m| m.name == "welcome")
        .unwrap()
        .text;
    assert!(
        body.contains("p/m") && body.contains(&dir.display().to_string()),
        "{body}"
    );
    t.close();

    common::cleanup(&dir);
}
