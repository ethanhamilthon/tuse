use tuse_core::{Options, Session};
#[test]
fn close_drains_hook_in_order_with_global_state() {
    let dir = std::env::temp_dir().join(format!("tuse-hook-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("events");
    let cfg = dir.join("main.yaml");
    std::fs::write(
        &cfg,
        format!(
            r#"functions:
  label:
    description: Label the current event
    run: return event
hooks:
  record:
    events: [session_start, session_end]
    run: |
      if event == 'session_start':
          n = 0
      n += 1
      with open('{}', 'a') as f:
          f.write(label() + str(n) + '\n')
      set_title('hook title')
"#,
            log.display()
        ),
    )
    .unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        resume: None,
        headless: true,
    })
    .unwrap();
    s.close();
    assert_eq!(
        std::fs::read_to_string(log).unwrap(),
        "session_start1\nsession_end2\n"
    );
    assert_eq!(s.title(), "hook title");
    std::fs::remove_dir_all(tuse_core::tuse_home().join(&s.id)).ok();
    std::fs::remove_dir_all(dir).unwrap();
}
