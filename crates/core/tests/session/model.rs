use std::path::Path;
use tuse_config::{ModelDef, Prices};
use tuse_core::{Options, Session};

fn open(cfg: &Path, cwd: &Path, resume: Option<String>) -> std::sync::Arc<Session> {
    Session::open(Options {
        cwd: cwd.to_path_buf(),
        config: Some(cfg.to_path_buf()),
        mode: tuse_core::Mode::Json,
        resume,
        start: Default::default(),
    })
    .unwrap()
}

#[test]
fn a_session_keeps_its_selection_and_reloads_model_metadata() {
    let dir = std::env::temp_dir().join(format!("tuse-model-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    unsafe {
        std::env::set_var("HOME", &home);
    }
    tuse_core::add_provider(&tuse_config::Connect {
        name: "p".into(),
        base_url: "http://127.0.0.1:1".into(),
        api_key: "k".into(),
        ..Default::default()
    })
    .unwrap();
    let cfg = dir.join("tuse.lua");
    let models = dir.join("models.lua");
    std::fs::write(
        &cfg,
        "return { models = require('models'), settings = { retries = 0 } }",
    )
    .unwrap();
    std::fs::write(
        &models,
        r#"return {{
        name = "p/a",
        display = "Model A",
        window = 200000,
        compact_at = 120000,
        prices = { input = 3, output = 15, cache_read = 0.3, cache_write = 3.75 },
    }}"#,
    )
    .unwrap();
    tuse_core::set_state("model", "p/a").unwrap();
    tuse_core::set_state("effort", "high").unwrap();
    let s = open(&cfg, &dir, None);
    assert_eq!(s.model(), ("p/a".into(), "high".into()));
    assert_eq!(s.available_models(), ["p/a"]);
    assert_eq!(s.snapshot(false).ctx_size, Some(200_000));
    assert_eq!(
        s.model_def("p/a"),
        Some(ModelDef {
            name: "p/a".into(),
            display: "Model A".into(),
            window: 200_000,
            compact_at: Some(120_000),
            prices: Some(Prices {
                input: 3.0,
                output: 15.0,
                cache_read: 0.3,
                cache_write: 3.75,
            }),
        })
    );
    let id = s.id.clone();
    let events = s.subscribe();
    s.submit(tuse_core::UserInput::text("hi"));
    while !matches!(
        events.recv_timeout(std::time::Duration::from_secs(10)),
        Ok(tuse_core::Event::TurnEnd { .. }) | Err(_)
    ) {}
    tuse_core::set_state("model", "p/b").unwrap();
    tuse_core::set_state("effort", "low").unwrap();
    std::fs::write(&models, "return {{ name = 'p/a', window = 300000 }}").unwrap();
    s.reload().unwrap();
    assert_eq!(s.model(), ("p/a".into(), "high".into()));
    assert_eq!(s.snapshot(false).ctx_size, Some(300_000));
    s.close();
    let again = open(&cfg, &dir, Some(id.clone()));
    assert_eq!(again.model(), ("p/a".into(), "high".into()));
    again.close();
    let fresh = open(&cfg, &dir, None);
    assert_eq!(fresh.model(), ("p/b".into(), "low".into()));
    fresh.close();
    assert!(!home.join(".tuse").join(&fresh.id).exists());
    assert!(
        tuse_core::list_sessions(&dir)
            .iter()
            .all(|m| m.id != fresh.id)
    );
    std::fs::remove_dir_all(dir).ok();
}
