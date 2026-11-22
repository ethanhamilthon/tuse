use std::path::Path;
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
fn a_session_keeps_its_model_when_the_config_changes() {
    let dir = std::env::temp_dir().join(format!("tuse-model-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: this file holds one test, so nothing else reads the environment meanwhile.
    unsafe {
        std::env::set_var("TUSE_HOME", dir.join("home"));
        std::env::remove_var("TUSE_PARENT");
    }
    let cfg = dir.join("tuse.lua");
    let write = |model: &str| {
        std::fs::write(&cfg, format!("return {{ model = 'p/{model}' }}")).unwrap();
    };
    write("a");
    let s = open(&cfg, &dir, None);
    assert_eq!(s.model().0, "p/a");
    let id = s.id.clone();
    // The session is stored with its first message; a turn that fails is enough.
    let events = s.subscribe();
    s.submit(tuse_core::UserInput::text("hi"));
    while !matches!(
        events.recv_timeout(std::time::Duration::from_secs(10)),
        Ok(tuse_core::Event::TurnEnd { .. }) | Err(_)
    ) {}
    write("b");
    s.reload().unwrap();
    assert_eq!(s.model().0, "p/a");
    s.close();
    let again = open(&cfg, &dir, Some(id.clone()));
    assert_eq!(again.model().0, "p/a");
    again.close();
    let fresh = open(&cfg, &dir, None);
    assert_eq!(fresh.model().0, "p/b");
    fresh.close();
    // A session closed before its first message leaves nothing.
    assert!(!dir.join("home").join(&fresh.id).exists());
    assert!(
        tuse_core::list_sessions(&dir)
            .iter()
            .all(|m| m.id != fresh.id)
    );
    std::fs::remove_dir_all(dir).ok();
}
