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
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: this file holds one test, so nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("TUSE_HOME", dir.join("home")) };
    let cfg = dir.join("tuse.yaml");
    let write = |model: &str| {
        let text = format!("model: p/{model}\nconnect: [{{name: p, base_url: x, api_key: k}}]\n");
        std::fs::write(&cfg, text).unwrap();
    };
    write("a");
    let s = open(&cfg, &dir, None);
    assert_eq!(s.model().0, "p/a");
    let id = s.id.clone();
    // `meta.json` appears with the first message; write it by hand so the session can be resumed.
    let meta = format!(
        r#"{{"id":"{id}","cwd":"{}","title":"t","model":"p/a","effort":"default","created":"","updated":""}}"#,
        dir.display()
    );
    let home = dir.join("home").join(&id);
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("meta.json"), meta).unwrap();
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
    // A session closed before its first message leaves no folder.
    assert!(!dir.join("home").join(&fresh.id).exists());
    std::fs::remove_dir_all(dir).ok();
}
