mod common;
use std::time::Duration;
use tuse_core::{Options, Session};

#[test]
fn a_child_closed_without_a_prompt_has_a_stable_elapsed() {
    let addr = common::provider(vec![]);
    let (dir, cfg) = common::setup("child-closed", &addr, "");
    // SAFETY: this file holds one test, so nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("TUSE_PARENT", "outer") };
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: tuse_core::Mode::Headless,
        resume: None,
        start: Default::default(),
    })
    .unwrap();
    let config = s.config();
    std::thread::sleep(Duration::from_millis(1500));
    s.close();
    let first = tuse_core::descendants("outer", &config);
    assert!(!first[0].running);
    std::thread::sleep(Duration::from_millis(2100));
    let second = tuse_core::descendants("outer", &config);
    assert_eq!(first[0].elapsed_secs, second[0].elapsed_secs);
    assert!(second[0].elapsed_secs >= 1);
    common::cleanup(&dir);
}
