use std::{fs, process::Command};

#[test]
fn models_lists_effective_lua_metadata_without_creating_a_session() {
    let dir = std::env::temp_dir().join(format!("tuse-cli-models-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let global = dir.join(".config/tuse");
    fs::create_dir_all(&global).unwrap();
    fs::write(
        global.join("init.lua"),
        "return function() return { base = { models = require('models') } } end",
    )
    .unwrap();
    fs::write(
        global.join("models.lua"),
        r#"return {{ name = "p/m", window = 200000,
        prices = { input = 2, output = 10 } }}"#,
    )
    .unwrap();
    fs::write(dir.join("tuse.lua"), "return {}").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tuse"))
            .current_dir(&dir)
            .env("HOME", &dir)
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
