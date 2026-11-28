use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn tuse(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(dir)
        .env("HOME", dir)
        .env_remove("TUSE_HOME")
        .env_remove("TUSE_GLOBAL_CONFIG")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn help_removed_commands_strict_flags_init_and_startup_exclusions() {
    let dir = std::env::temp_dir().join(format!("tuse-cli-flags-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tuse.lua"), "return { bad = { typo_key = 1 } }").unwrap();

    let out = tuse(&dir, &["json", "--help"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.starts_with("tuse json [-r|--resume ID]")
            && text.contains("--no-global")
            && !text.contains("--no-local")
            && text.contains("--no-plugins"),
        "{text}"
    );

    let out = tuse(&dir, &["help", "json"]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), text);

    let out = tuse(&dir, &["help", "rollback"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "tuse rollback"
    );

    let out = tuse(&dir, &["providers", "--bogus"]);
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("unknown flag --bogus"), "{err}");

    for args in [
        &["connect"][..],
        &["disconnect", "x"],
        &["plugins", "list"],
        &["help", "connect"],
        &["help", "disconnect"],
        &["help", "plugins"],
        &["models", "extra"],
        &["init", "x"],
        &["help", "bogus"],
        &["help", "json", "extra"],
        &["version", "--version"],
        &["--version", "extra"],
        &["providers", "--json=false"],
        &["--no-global=false"],
        &["json", "--no-local=true"],
        &["json", "--no-plugins=1"],
        &["-c", "--resume", "abc"],
    ] {
        assert!(!tuse(&dir, args).status.success(), "{args:?}");
    }
    assert!(!dir.join(".tuse").exists(), "nothing ran");

    let out = tuse(&dir, &["providers", "--json"]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "[]");

    let home_dir = dir.join(".tuse");
    unsafe {
        std::env::set_var("TUSE_HOME", &home_dir);
    }
    let mut hdrs = std::collections::BTreeMap::new();
    hdrs.insert("x-custom-secret".into(), "hdr-secret-value".into());
    tuse_core::Providers::add(&tuse_config::Connect {
        name: "cli_prov".into(),
        base_url: "https://example.invalid/v1".into(),
        api_key: "sk-cli-secret-token".into(),
        api: "anthropic-messages".into(),
        headers: hdrs,
    })
    .unwrap();
    unsafe {
        std::env::remove_var("TUSE_HOME");
    }
    let text_out = tuse(&dir, &["providers"]);
    assert!(text_out.status.success());
    let text_str = String::from_utf8(text_out.stdout).unwrap();
    assert!(
        text_str.contains("cli_prov\thttps://example.invalid/v1"),
        "{text_str}"
    );
    for leak in ["sk-cli-secret-token", "hdr-secret-value", "x-custom-secret"] {
        assert!(!text_str.contains(leak), "{text_str}");
    }
    let json_out = tuse(&dir, &["providers", "--json"]);
    assert!(json_out.status.success());
    let json_str = String::from_utf8(json_out.stdout).unwrap();
    let arr: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let obj = arr[0].as_object().unwrap();
    assert_eq!(obj.len(), 3);
    assert_eq!(obj["name"], "cli_prov");
    assert_eq!(obj["type"], "anthropic-messages");
    assert_eq!(obj["base"], "https://example.invalid/v1");
    for leak in [
        "sk-cli-secret-token",
        "hdr-secret-value",
        "x-custom-secret",
        "headers",
        "api_key",
        "base_url",
    ] {
        assert!(!json_str.contains(leak), "{json_str}");
    }

    assert!(!tuse(&dir, &["models", "-c", "custom.lua"]).status.success());

    let proj = dir.join("init-proj");
    std::fs::create_dir_all(&proj).unwrap();
    let init_out = tuse(&proj, &["init"]);
    assert!(!init_out.status.success());
    assert!(!proj.join("tuse.lua").exists());
    assert!(!proj.join("AGENTS.md").exists());
    assert!(!tuse(&proj, &["help", "init"]).status.success());

    let global_str = dir.join("global-str.lua");
    std::fs::write(&global_str, "return { systems = 'GLOBAL STRING KEPT' }").unwrap();
    let mut ch = Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(&proj)
        .env("HOME", &dir)
        .env_remove("TUSE_HOME")
        .env("TUSE_GLOBAL_CONFIG", &global_str)
        .args(["json", "--no-plugins"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut sin = ch.stdin.take().unwrap();
    let mut sout = BufReader::new(ch.stdout.take().unwrap());
    let mut req = |id: u64, method: &str| -> serde_json::Value {
        writeln!(
            sin,
            r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{{}}}}"#
        )
        .unwrap();
        sin.flush().unwrap();
        loop {
            let mut line = String::new();
            sout.read_line(&mut line).unwrap();
            let v: serde_json::Value = serde_json::from_str(&line).unwrap();
            if v["id"] == id {
                return v;
            }
        }
    };
    assert_eq!(req(1, "initialize")["result"]["protocolVersion"], 2);
    let h1 = req(2, "session.history")["result"].clone();
    assert_eq!(h1[0]["text"], "GLOBAL STRING KEPT");

    std::fs::write(proj.join("tuse.lua"), "return 'LIVE AGENTS INSTRUCTIONS'").unwrap();
    std::fs::write(
        &global_str,
        "return { systems = assert(loadfile('./tuse.lua'))() }",
    )
    .unwrap();
    assert!(req(3, "config.reload")["error"].is_null());
    let h2 = req(4, "session.history")["result"].clone();
    let h2_text = h2[0]["text"].as_str().unwrap();
    assert!(
        h2_text.contains("LIVE AGENTS INSTRUCTIONS") && !h2_text.contains("GLOBAL STRING KEPT"),
        "{h2_text}"
    );

    req(5, "shutdown");
    drop(sin);
    assert!(ch.wait().unwrap().success());

    std::fs::create_dir_all(dir.join(".config/tuse")).unwrap();
    std::fs::write(dir.join(".config/tuse/init.lua"), "error('broken global')").unwrap();
    let mut ch2 = Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(&dir)
        .env("HOME", &dir)
        .env_remove("TUSE_HOME")
        .env_remove("TUSE_GLOBAL_CONFIG")
        .args(["json", "--no-global", "--no-plugins"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut sin2 = ch2.stdin.take().unwrap();
    writeln!(
        sin2,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
    )
    .unwrap();
    writeln!(
        sin2,
        r#"{{"jsonrpc":"2.0","id":2,"method":"shutdown","params":{{}}}}"#
    )
    .unwrap();
    drop(sin2);
    let out2 = ch2.wait_with_output().unwrap();
    assert!(
        out2.status.success(),
        "{}",
        String::from_utf8_lossy(&out2.stderr)
    );

    std::fs::remove_dir_all(dir).unwrap();
}
