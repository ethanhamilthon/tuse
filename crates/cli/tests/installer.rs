use std::path::Path;
use std::process::Command;

#[test]
fn installer_handles_prompts_races_signals_and_capability_gate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .arg(root.join("release/install_test.py"))
        .env("TUSE_BIN", env!("CARGO_BIN_EXE_tuse"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
