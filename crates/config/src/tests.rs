use crate::{expand::vars, load, load::resolve_command};
use std::path::Path;

#[test]
fn vars_expand() {
    let get = |n: &str| if n == "A" { "1".into() } else { String::new() };
    assert_eq!(vars("x$A ${A}y $B $$ $1", get), "x1 1y  $ $1");
}

#[test]
fn command_paths() {
    let d = Path::new("/cfg");
    assert_eq!(resolve_command(".bin/hook -v", d), "/cfg/.bin/hook -v");
    assert_eq!(resolve_command("notify-send hi", d), "notify-send hi");
    assert_eq!(resolve_command("/usr/bin/x", d), "/usr/bin/x");
}

#[test]
fn includes_merge() {
    let dir = std::env::temp_dir().join(format!("tuse-cfg-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let w = |n: &str, s: &str| std::fs::write(dir.join(n), s).unwrap();
    w(
        "sub/a.yaml",
        "model: a\neffort: low\nsystems: [one]\nhooks: {h: {events: [turn_start], run: ./h.sh}}\n",
    );
    w(
        "b.yaml",
        "model: b\nsystems: [two]\nsettings: {bash_timeout: 5}\n",
    );
    w(
        "main.yaml",
        "include: [sub/a.yaml, b.yaml]\neffort: high\nsettings: {max_bash_output: 10}\nprompts: {p: \"#{{echo hi}}\"}\n",
    );
    let c = load(Some(&dir.join("main.yaml"))).unwrap().config;
    assert_eq!(c.model, "b");
    assert_eq!(c.effort, "high");
    assert_eq!(c.systems, ["one", "two"]);
    assert_eq!(c.settings.bash_timeout, 5);
    assert_eq!(c.settings.max_bash_output, 10);
    assert_eq!(c.prompts["p"], "hi");
    assert_eq!(
        c.hooks["h"].run,
        dir.join("sub/./h.sh").display().to_string()
    );
    std::fs::remove_dir_all(dir).ok();
}
