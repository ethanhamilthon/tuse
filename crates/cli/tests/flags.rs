use std::process::{Command, Output};

fn tuse(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tuse"))
        .current_dir(dir)
        .env("HOME", dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn help_comes_first_and_unknown_input_is_rejected() {
    let dir = std::env::temp_dir().join(format!("tuse-cli-flags-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tuse.lua"), "return { typo_key = 1 }").unwrap();

    let out = tuse(&dir, &["json", "--help"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("tuse json [-c|--config PATH]"), "{text}");

    let out = tuse(&dir, &["providers", "--bogus"]);
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("unknown flag --bogus"), "{err}");

    for args in [&["models", "extra"][..], &["init", "x"], &["disconnect"]] {
        assert!(!tuse(&dir, args).status.success(), "{args:?}");
    }
    assert!(!dir.join(".tuse").exists(), "nothing ran");

    std::fs::write(
        dir.join("other.lua"),
        "return { models = {{ name = 'x/y', window = 1000 }} }",
    )
    .unwrap();
    let out = tuse(&dir, &["models", "-c", "other.lua"]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        out.status.success() && text.contains("x/y\t1.0k ctx"),
        "{text}"
    );
    let out = tuse(&dir, &["models", "--config=other.lua"]);
    assert!(out.status.success());
    std::fs::remove_dir_all(dir).unwrap();
}
