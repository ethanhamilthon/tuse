use serde_json::Value;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn flat_plugins_and_user_namespaces_share_one_registry() {
    let root = std::env::temp_dir().join(format!("tuse-config-namespaces-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let home = root.join("state");
    let config = root.join("config.lua");
    fs::create_dir_all(home.join("plugins")).unwrap();
    let install = |name: &str, manifest: &str, code: &str| {
        let dir = home.join("plugins").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("tuse-plugin.json"), manifest).unwrap();
        fs::write(dir.join("init.lua"), code).unwrap();
    };
    install(
        "demo",
        r#"{"version":"1"}"#,
        "return {prompts={review='plugin'}, functions={ping=function() end}}",
    );
    install(
        "lazy",
        r#"{"version":"1","lazy":true}"#,
        "return {prompts={review='lazy'}}",
    );
    install(
        "broken",
        r#"{"version":"1","lazy":true}"#,
        "error('broken entrypoint')",
    );
    install(
        "library",
        r#"{"version":"1","lazy":true,"entrypoint":"lib.lua"}"#,
        "error('default entrypoint must not run')",
    );
    fs::write(
        home.join("plugins/library/lib.lua"),
        "return function() return 42 end",
    )
    .unwrap();
    fs::write(
        root.join("tuse.lua"),
        "error('project config must not run')",
    )
    .unwrap();
    fs::write(
        &config,
        r#"
        local plugins = require('tuse').plugins
        assert(plugins.get('lazy').manifest.lazy)
        assert(plugins.get('broken').name == 'broken')
        local ok, err = pcall(plugins.load, 'broken')
        assert(not ok and err:find('broken entrypoint', 1, true))
        ok, err = pcall(plugins.load, 'missing')
        assert(not ok and err:find('not installed', 1, true))
        local entry = plugins.load('library')
        assert(type(entry) == 'function' and entry() == 42)
        return {
            models={['p/m']={efforts={}}},
            work={prompts={review='user'}, functions={ping=function() end}},
            tools=plugins.load('lazy'),
        }
    "#,
    )
    .unwrap();
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_tuse"))
            .current_dir(&root)
            .env("TUSE_HOME", &home)
            .env("TUSE_GLOBAL_CONFIG", &config)
            .arg("json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let mut child = spawn();
    let mut stdin = child.stdin.take().unwrap();
    for (id, method) in [
        (0, "initialize"),
        (1, "config.get"),
        (2, "config.reload"),
        (3, "config.reload"),
        (4, "shutdown"),
    ] {
        writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0", "id":id,"method":method,"params":if id == 3 { serde_json::json!({"ns":"work"}) } else { serde_json::json!({}) }})).unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let cfg = &frames.iter().find(|v| v["id"] == 1).unwrap()["result"];
    assert_eq!(cfg["prompts"]["work:review"], "user");
    assert_eq!(cfg["prompts"]["demo:review"], "plugin");
    assert_eq!(cfg["prompts"]["tools:review"], "lazy");
    assert!(cfg["prompts"].get("lazy:review").is_none());
    assert_eq!(cfg["functions"]["demo.ping"], "custom");
    assert_eq!(cfg["models"]["p/m"]["efforts"], serde_json::json!([]));
    assert!(
        frames
            .iter()
            .any(|v| v["id"] == 2 && v.get("result").is_some())
    );

    assert!(
        frames
            .iter()
            .any(|v| v["id"] == 3 && v["error"]["code"] == -32602)
    );
    fs::write(&config, "return { demo = { prompts={another='user'} } }").unwrap();
    let out = spawn().wait_with_output().unwrap();
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(!out.status.success(), "{err}");
    assert!(
        err.contains("duplicate namespace `demo`")
            && err.contains("config.lua")
            && err.contains("plugin:demo"),
        "{err}"
    );
    fs::write(&config, "return {}").unwrap();
    install(
        "old",
        r#"{"version":"1"}"#,
        "return function() error('must not be called') end",
    );
    let out = spawn().wait_with_output().unwrap();
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        !out.status.success()
            && err.contains("plugin:old")
            && err.contains("expected a table")
            && !err.contains("must not be called"),
        "{err}"
    );
    fs::remove_dir_all(&root).unwrap();
}
