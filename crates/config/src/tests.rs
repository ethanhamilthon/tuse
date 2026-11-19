use crate::expand::vars;
use std::path::Path;

fn load(p: Option<&Path>) -> Result<crate::Loaded, String> {
    crate::load(p)
}

#[test]
fn vars_expand() {
    let get = |n: &str| if n == "A" { "1".into() } else { String::new() };
    assert_eq!(vars("x$A ${A}y $B $$ $1", get), "x1 1y  $ $1");
}

#[test]
fn includes_merge() {
    let dir = std::env::temp_dir().join(format!("tuse-cfg-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let w = |n: &str, s: &str| std::fs::write(dir.join(n), s).unwrap();
    w(
        "sub/a.yaml",
        "model: a\neffort: low\nsystems: [one]\nhooks: {h: {events: [turn_start], run: 'sh(\"./h.sh\")'}}\n",
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
    assert_eq!(c.settings.tool_timeout, 5);
    assert_eq!(c.settings.max_output, 10);
    assert_eq!(c.prompts["p"], "hi");
    assert_eq!(c.settings.hook_timeout, 30);
    assert_eq!(c.hooks["h"].run, "sh(\"./h.sh\")");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn theme_extend() {
    let dir = std::env::temp_dir().join(format!("tuse-theme-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("main.yaml");
    std::fs::write(
        &p,
        "theme: b\nthemes:\n  a: {accent: red, bash_line: -1}\n  b: {extend: a, accent: blue}\n",
    )
    .unwrap();
    let t = load(Some(&p)).unwrap().config.active_theme();
    assert_eq!((t.accent.as_str(), t.tool_line), ("blue", -1));
    std::fs::write(&p, "theme: nope\n").unwrap();
    assert!(load(Some(&p)).is_err());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn line_limits() {
    use crate::limit;
    assert_eq!(
        (limit(0, 5), limit(-1, 5), limit(3, 5), limit(9, 5)),
        (0, 5, 3, 5)
    );
}

#[test]
fn set_key_keeps_the_rest() {
    let p = std::env::temp_dir().join(format!("tuse-key-{}.yaml", std::process::id()));
    std::fs::write(&p, "# top\nmodel: a\ntheme:\n  x: 1\nsystems:\n  - s\n").unwrap();
    crate::set_key(&p, "theme", "my theme").unwrap();
    crate::set_key(&p, "model", "x/b").unwrap();
    crate::set_key(&p, "effort", "high").unwrap();
    let s = std::fs::read_to_string(&p).unwrap();
    assert_eq!(
        s,
        "# top\nmodel: x/b\ntheme: \"my theme\"\nsystems:\n  - s\neffort: high\n"
    );
    std::fs::remove_file(p).ok();
}

#[test]
fn add_connect_appends() {
    let p = std::env::temp_dir().join(format!("tuse-conn-{}.yaml", std::process::id()));
    std::fs::write(
        &p,
        "model: a\nconnect:\n  - name: x\n    base_url: u\n\nsettings: {}\n",
    )
    .unwrap();
    crate::add_connect(&p, "y", "http://h/v1", "$KEY").unwrap();
    let c = load(Some(&p)).unwrap().config;
    assert_eq!(c.connect.len(), 2);
    assert_eq!(c.connect[1].name, "y");
    assert_eq!(c.connect[1].base_url, "http://h/v1");
    std::fs::remove_file(&p).ok();
    crate::add_connect(&p, "z", "http://h", "k").unwrap();
    assert_eq!(load(Some(&p)).unwrap().config.connect[0].name, "z");
    std::fs::remove_file(p).ok();
}

#[test]
fn find_order() {
    let dir = std::env::temp_dir().join(format!("tuse-find-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("cwd")).unwrap();
    let (cwd, global, flag) = (dir.join("cwd"), dir.join("g.yaml"), dir.join("f.yaml"));
    let find = |explicit: Option<&Path>| crate::load::find_in(explicit, &cwd, &global);
    assert_eq!(find(None), None);
    std::fs::write(&global, "").unwrap();
    assert_eq!(find(None), Some(global.clone()));
    std::fs::write(cwd.join("tuse.yaml"), "").unwrap();
    assert_eq!(find(None), Some(cwd.join("tuse.yaml")));
    assert_eq!(find(Some(&flag)), Some(flag));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn settings_theme_wins_and_is_written() {
    let p = std::env::temp_dir().join(format!("tuse-st-{}.yaml", std::process::id()));
    std::fs::write(
        &p,
        "theme: a\nthemes: {a: {}, b: {}}\nsettings:\n  retries: 2\n",
    )
    .unwrap();
    assert_eq!(load(Some(&p)).unwrap().config.theme_name(), "a");
    crate::set_setting(&p, "theme", "b").unwrap();
    crate::set_setting(&p, "theme", "b").unwrap();
    let s = std::fs::read_to_string(&p).unwrap();
    assert_eq!(
        s,
        "theme: a\nthemes: {a: {}, b: {}}\nsettings:\n  theme: b\n  retries: 2\n"
    );
    assert_eq!(load(Some(&p)).unwrap().config.theme_name(), "b");
    std::fs::remove_file(p).ok();
}

#[test]
fn shipped_main_includes_default() {
    let dir = std::env::temp_dir().join(format!("tuse-ship-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let release = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../release");
    for f in ["main.yaml", "default.yaml"] {
        std::fs::copy(release.join(f), dir.join(f)).unwrap();
    }
    let c = load(Some(&dir.join("main.yaml"))).unwrap().config;
    assert!(c.systems[0].contains("running inside Tuse"));
    assert_eq!(c.settings.retries, 5);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn theme_input_tokens_are_checked() {
    let p = std::env::temp_dir().join(format!("tuse-tok-{}.yaml", std::process::id()));
    std::fs::write(
        &p,
        "themes:\n  a: {input_design: box, motion: false, input_prefix: \"\"}\n",
    )
    .unwrap();
    let t = load(Some(&p)).unwrap().config.themes["a"].clone();
    assert_eq!(
        (t.input_design.as_str(), t.motion, t.input_prefix.as_str()),
        ("box", false, "")
    );
    std::fs::write(&p, "themes:\n  a: {input_design: round}\n").unwrap();
    assert!(load(Some(&p)).err().unwrap().contains("input_design"));
    std::fs::write(&p, "themes:\n  a: {box_chars: [x]}\n").unwrap();
    assert!(load(Some(&p)).err().unwrap().contains("box_chars"));
    std::fs::remove_file(p).ok();
}
