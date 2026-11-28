use std::{fs, process::Command};

#[test]
fn models_lists_effective_lua_metadata_without_creating_a_session() {
    let dir = std::env::temp_dir().join(format!("tuse-cli-models-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let global = dir.join(".config/tuse");
    fs::create_dir_all(&global).unwrap();
    fs::write(
        global.join("init.lua"),
        "return { models = require('models') }",
    )
    .unwrap();
    fs::write(
        global.join("models.lua"),
        r#"return { ["p/m"] = {window = 200000,
        prices = { input = 2, output = 10 } }}"#,
    )
    .unwrap();
    fs::write(dir.join("tuse.lua"), "return {}").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tuse"))
            .current_dir(&dir)
            .env("HOME", &dir)
            .env_remove("TUSE_HOME")
            .env_remove("TUSE_GLOBAL_CONFIG")
            .arg("models")
            .args(args)
            .output()
            .unwrap()
    };
    let out = run(&[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("p/m\t200k ctx · $2/$10"), "{text}");
    assert!(!run(&["import"]).status.success());
    assert!(!run(&["list"]).status.success());
    assert!(
        fs::read_dir(dir.join(".tuse"))
            .unwrap()
            .flatten()
            .all(|e| e.path().is_file())
    );
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn tusedev_uses_separate_storage_and_global_config() {
    let dir = std::env::temp_dir().join(format!("tuse-cli-dev-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let config = dir.join(".config/tusedev");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("init.lua"),
        "return { models = { ['dev/model'] = {} } }",
    )
    .unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_tuse"), dir.join(".tusedev-bin")).unwrap();
    let launcher = dir.join("tusedev");
    fs::write(&launcher, include_str!("../../../release/tusedev.sh")).unwrap();
    let out = Command::new("sh")
        .arg(launcher)
        .arg("models")
        .current_dir(&dir)
        .env("HOME", &dir)
        .env("TUSE_HOME", dir.join(".tuse"))
        .env("TUSE_GLOBAL_CONFIG", dir.join("not-a-dev-config.lua"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("dev/model"));
    assert!(dir.join(".tusedev/tuse.db").is_file());
    assert!(!dir.join(".tuse").exists());
    fs::remove_dir_all(dir).unwrap();
}
