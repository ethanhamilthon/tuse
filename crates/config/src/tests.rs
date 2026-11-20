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
        "model: a\neffort: low\nsystems: [one]\nhooks: {h: {events: [user_message], run: 'sh(\"./h.sh\")'}}\n",
    );
    w(
        "b.yaml",
        "model: b\nsystems: [two]\nsettings: {tool_timeout: 5}\n",
    );
    w(
        "main.yaml",
        "include: [sub/a.yaml, b.yaml]\neffort: high\nsettings: {max_output: 10}\nprompts: {p: \"#{{echo hi}}\"}\n",
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
    assert!(c.problems().is_empty());
    w(
        "main.yaml",
        "models: [{name: p/a, window: 1000}, {name: p/b, window: 1000, compact_at: 0}]\n",
    );
    let c = load(Some(&dir.join("main.yaml"))).unwrap().config;
    assert_eq!(
        (
            c.compact_at("p/a"),
            c.compact_at("p/b"),
            c.compact_at("p/x")
        ),
        (600, 0, 0)
    );
    w("main.yaml", "effort: hgih\n");
    let c = load(Some(&dir.join("main.yaml"))).unwrap().config;
    assert_eq!(c.default_effort(), "default");
    assert!(
        c.problems()[0].contains("unknown effort `hgih`"),
        "{:?}",
        c.problems()
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn theme_extend() {
    let dir = std::env::temp_dir().join(format!("tuse-theme-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("main.yaml");
    std::fs::write(
        &p,
        "settings:\n  theme: b\n  ui: {tool_line: 7}\nthemes:\n  a: {accent: red, info: green}\n  b: {extend: a, accent: blue}\n",
    )
    .unwrap();
    let cfg = load(Some(&p)).unwrap().config;
    let t = cfg.active_theme();
    assert_eq!(
        (
            t.accent.as_str(),
            t.info.as_str(),
            cfg.settings.ui.tool_line
        ),
        ("blue", "green", 7)
    );
    std::fs::write(&p, "settings: {theme: nope}\n").unwrap();
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
fn settings_theme_is_written() {
    let p = std::env::temp_dir().join(format!("tuse-st-{}.yaml", std::process::id()));
    std::fs::write(&p, "themes: {a: {}, b: {}}\nsettings:\n  retries: 2\n").unwrap();
    crate::set_setting(&p, "theme", "b").unwrap();
    crate::set_setting(&p, "theme", "b").unwrap();
    let s = std::fs::read_to_string(&p).unwrap();
    assert_eq!(
        s,
        "themes: {a: {}, b: {}}\nsettings:\n  theme: b\n  retries: 2\n"
    );
    assert_eq!(load(Some(&p)).unwrap().config.theme_name(), "b");
    std::fs::remove_file(p).ok();
}

#[test]
fn defaults_are_what_the_docs_promise() {
    let s = crate::Settings::default();
    assert_eq!(
        (s.compact_at, s.max_output, s.tool_timeout, s.hook_timeout),
        (None, 4096, 10, 30)
    );
    assert_eq!((s.max_image_bytes, s.retries), (5 * 1024 * 1024, 5));
    let ui = &s.ui;
    assert_eq!(
        (ui.input_max_height, ui.tool_line, ui.output_line),
        (-1, -1, 1)
    );
    assert_eq!(
        (ui.reasoning_line, ui.diff_line, ui.pad_x, ui.message_gap),
        (-1, 12, 0, 1)
    );
    assert_eq!((ui.input_design.as_str(), ui.motion), ("lines", true));
    let t = crate::Theme::default();
    assert_eq!(
        (t.bg.as_str(), t.surface.as_str(), t.fg.as_str()),
        ("#0A0A0A", "#1A1A1A", "#EDEDED")
    );
}

#[test]
fn ui_settings_are_checked() {
    let p = std::env::temp_dir().join(format!("tuse-tok-{}.yaml", std::process::id()));
    std::fs::write(&p, "settings:\n  ui: {input_design: box, motion: false}\n").unwrap();
    let ui = load(Some(&p)).unwrap().config.settings.ui;
    assert_eq!((ui.input_design.as_str(), ui.motion), ("box", false));
    std::fs::write(&p, "settings:\n  ui: {input_design: round}\n").unwrap();
    assert!(load(Some(&p)).err().unwrap().contains("input_design"));
    std::fs::remove_file(p).ok();
}

#[test]
fn effective_yaml_contains_default_theme_and_settings() {
    let c = crate::load(None).unwrap().config;
    let value: serde_yaml::Value = serde_yaml::from_str(&c.to_yaml().unwrap()).unwrap();
    assert_eq!(value["themes"]["default"]["bg"], "#0A0A0A");
    assert_eq!(value["settings"]["ui"]["motion"], true);
}

#[test]
fn rejects_removed_prelude() {
    let path = std::env::temp_dir().join(format!("tuse-prelude-{}.yaml", std::process::id()));
    std::fs::write(&path, "prelude: ['pass']").unwrap();
    let err = load(Some(&path)).err().unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(err.contains("`prelude` is no longer supported; use hooks or `import`"));
}
