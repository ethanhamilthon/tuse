use crate::{Mode, load};
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tuse-cfg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("sub")).unwrap();
    d
}

#[test]
fn include_only_filters_after_nested_includes() {
    let d = dir("only");
    let w = |n: &str, s: &str| std::fs::write(d.join(n), s).unwrap();
    w(
        "sub/inner.yaml",
        "connect: [{name: a, base_url: http://a, api_key: k}]\nmodel: a/m\nsystems: [from-inner]\n",
    );
    w(
        "shared.yaml",
        "include: [sub/inner.yaml]\nconnect: [{name: b, base_url: http://b, api_key: k}]\neffort: low\n",
    );
    w("base.yaml", "effort: medium\nsystems: [base]\n");
    w(
        "main.yaml",
        &format!(
            "include:\n  - base.yaml\n  - file: {}\n    only: [connect, missing_key]\n",
            d.join("shared.yaml").display()
        ),
    );
    let c = load(Some(&d.join("main.yaml")), Mode::Tui).unwrap().config;
    assert_eq!(
        c.connect
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(c.model, "");
    assert_eq!(c.effort, "medium");
    assert_eq!(
        c.systems.iter().map(|s| s.text()).collect::<Vec<_>>(),
        ["base"]
    );

    // Explicit `only: []` drops every key; `except` is rejected.
    w("main.yaml", "include:\n  - file: base.yaml\n    only: []\n");
    let c = load(Some(&d.join("main.yaml")), Mode::Tui).unwrap().config;
    assert_eq!(c.effort, "");
    assert!(c.systems.is_empty());

    w(
        "main.yaml",
        "include:\n  - file: base.yaml\n    except: [systems]\n",
    );
    assert!(load(Some(&d.join("main.yaml")), Mode::Tui).is_err());
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn include_modes_skips_non_matching_files() {
    let d = dir("inc-modes");
    let w = |n: &str, s: &str| std::fs::write(d.join(n), s).unwrap();
    w("tui.yaml", "effort: high\nsystems: [tui-sys]\n");
    w("shared.yaml", "effort: low\nsystems: [shared-sys]\n");
    w(
        "main.yaml",
        "include:\n  - file: tui.yaml\n    modes: [tui]\n  - file: missing-json.yaml\n    modes: [json]\n  - file: shared.yaml\n    only: [systems]\n    modes: [tui, headless]\n",
    );

    let tui = load(Some(&d.join("main.yaml")), Mode::Tui).unwrap();
    assert_eq!(tui.config.effort, "high");
    assert_eq!(
        tui.config
            .systems
            .iter()
            .map(|s| s.text())
            .collect::<Vec<_>>(),
        ["tui-sys", "shared-sys"]
    );
    assert_eq!(tui.files.len(), 3);

    let headless = load(Some(&d.join("main.yaml")), Mode::Headless).unwrap();
    assert_eq!(headless.config.effort, "");
    assert_eq!(
        headless
            .config
            .systems
            .iter()
            .map(|s| s.text())
            .collect::<Vec<_>>(),
        ["shared-sys"]
    );
    assert_eq!(headless.files.len(), 2);

    // In json mode the missing file IS attempted and fails.
    assert!(load(Some(&d.join("main.yaml")), Mode::Json).is_err());
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn modes_on_entries_and_connect_lookup() {
    let d = dir("entry-modes");
    let p = d.join("main.yaml");
    std::fs::write(
        &p,
        r#"model: m
connect:
  - {name: gw, base_url: http://tui, api_key: k, modes: [tui]}
  - {name: gw, base_url: http://hl, api_key: k, modes: [headless]}
  - {name: fallback, base_url: http://all, api_key: k, modes: []}
systems:
  - plain
  - {text: "only tui #{{echo ok}}", modes: [tui]}
  - {text: all-explicit}
prompts:
  p1: plain prompt
  p2: {text: tui prompt, modes: [tui, json]}
functions:
  f_all: {description: a, run: "return 1"}
  f_hl: {description: b, run: "return 2", modes: [headless]}
hooks:
  h_tui: {events: [user_message], run: "pass", modes: [tui]}
"#,
    )
    .unwrap();
    let c = load(Some(&p), Mode::Tui).unwrap().config;

    assert_eq!(c.systems[0].text(), "plain");
    assert!(c.systems[0].matches(Mode::Headless));
    assert_eq!(c.systems[1].text(), "only tui ok");
    assert!(c.systems[1].matches(Mode::Tui));
    assert!(!c.systems[1].matches(Mode::Headless));
    assert!(c.systems[2].matches(Mode::Json));

    assert_eq!(c.prompts["p1"].text(), "plain prompt");
    assert!(c.prompts["p1"].matches(Mode::Headless));
    assert!(c.prompts["p2"].matches(Mode::Tui));
    assert!(c.prompts["p2"].matches(Mode::Json));
    assert!(!c.prompts["p2"].matches(Mode::Headless));

    assert!(c.functions["f_all"].matches(Mode::Json));
    assert!(c.functions["f_hl"].matches(Mode::Headless));
    assert!(!c.functions["f_hl"].matches(Mode::Tui));
    assert!(c.hooks["h_tui"].matches(Mode::Tui));
    assert!(!c.hooks["h_tui"].matches(Mode::Json));

    assert_eq!(c.connect("gw", Mode::Tui).unwrap().base_url, "http://tui");
    assert_eq!(
        c.connect("gw", Mode::Headless).unwrap().base_url,
        "http://hl"
    );
    assert!(c.connect("gw", Mode::Json).is_none());
    assert_eq!(c.default_model(Mode::Tui), "gw/m");
    assert_eq!(c.default_model(Mode::Headless), "gw/m");
    assert_eq!(c.default_model(Mode::Json), "fallback/m");

    // Explicit provider prefix stays intact even when excluded in the current mode.
    std::fs::write(
        &p,
        "model: a/m\nconnect:\n  - {name: a, base_url: http://a, api_key: k, modes: [tui]}\n  - {name: b, base_url: http://b, api_key: k, modes: [headless]}\n",
    )
    .unwrap();
    let c = load(Some(&p), Mode::Headless).unwrap().config;
    assert_eq!(c.default_model(Mode::Headless), "a/m");
    assert!(c.connect("a", Mode::Headless).is_none());

    std::fs::remove_dir_all(d).ok();
}

#[test]
fn rejects_unknown_mode_everywhere() {
    let d = dir("bad-modes");
    let p = d.join("main.yaml");
    for yaml in [
        "include: [{file: a.yaml, modes: [bogus]}]\n",
        "hooks: {h: {events: [user_message], run: pass, modes: [bogus]}}\n",
        "systems: [{text: hi, modes: [bogus]}]\n",
        "prompts: {p: {text: hi, modes: [bogus]}}\n",
        "functions: {f: {description: d, run: pass, modes: [bogus]}}\n",
        "connect: [{name: a, base_url: http://a, api_key: k, modes: [bogus]}]\n",
    ] {
        std::fs::write(&p, yaml).unwrap();
        let err = load(Some(&p), Mode::Tui).err().expect(yaml);
        assert!(err.contains("bogus"), "yaml={yaml} err={err}");
    }
    std::fs::remove_dir_all(d).ok();
}
