mod common;

use std::fs;
use tuse_core::{Mode, Options, Session};

#[test]
fn reload_restores_preloaded_modules_and_callbacks_after_a_failed_candidate() {
    let (dir, cfg) = common::setup("require-rollback", "localhost:1", "");
    unsafe { std::env::set_var("TUSE_GLOBAL_CONFIG", &cfg) };
    let valid = r#"
local t = require('tuse')
assert(sh == nil and fs == nil and type(t.fs.read) == 'function')
assert(t.session.get().config_path:match("/tuse.lua$"))
package.preload.helper = function() return 'before' end
assert(require('helper') == 'before')
return { work = { commands = { ping = function() t.ui.show('probe', require('helper')) end } } }
"#;
    fs::write(&cfg, valid).unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        mode: Mode::Json,
        resume: None,
        flags: Default::default(),
        start: Default::default(),
    })
    .unwrap();
    fs::write(&cfg, "package.preload.helper = function() return 'bad' end; require('helper'); error('rollback')").unwrap();
    assert!(s.reload().unwrap_err().contains("rollback"));
    let rx = s.subscribe();
    s.run_command("work:ping", &[]).unwrap();
    let observed =
        (0..10).find_map(
            |_| match rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap() {
                tuse_core::Event::UiMarkdown { text, .. } => Some(text),
                _ => None,
            },
        );
    assert_eq!(observed.as_deref(), Some("before"));
    s.close();
    common::cleanup(&dir);
}
