mod common;

use std::fs;
use tuse_core::{Mode, Options, Session, Start};

#[test]
fn documented_configs_load() {
    let addr = common::provider(vec![]);
    let (dir, cfg) = common::setup("documented-configs", &addr, "");
    fs::write(dir.join("AGENTS.md"), "Test project instructions.").unwrap();
    unsafe { std::env::set_var("TUSE_GLOBAL_CONFIG", &cfg) };

    for (page, text, count) in [
        ("README.md", include_str!("../../../README.md"), 4),
        (
            "docs/examples.md",
            include_str!("../../../docs/examples.md"),
            2,
        ),
    ] {
        let snippets: Vec<_> = text
            .split("```lua\n")
            .skip(1)
            .map(|block| block.split_once("\n```").unwrap().0)
            .collect();
        assert_eq!(snippets.len(), count, "{page}");
        for (index, snippet) in snippets.into_iter().enumerate() {
            fs::write(&cfg, snippet).unwrap();
            let session = Session::open(Options {
                cwd: dir.clone(),
                mode: Mode::Json,
                resume: None,
                start: Start::Launch,
                flags: Default::default(),
            })
            .unwrap_or_else(|e| panic!("{page}, Lua snippet {}: {e}", index + 1));
            if page == "docs/examples.md" && index == 0 {
                let config = session.config();
                assert!(config.prompts.contains_key("work:review"));
                assert!(config.functions.contains_key("work.status"));
                assert!(config.commands.contains("work:diff"));
                let tuse_config::Systems::Named(systems) = &config.systems else {
                    panic!("expected named systems");
                };
                assert_eq!(systems["project"], "Test project instructions.");
            }
            session.close();
        }
    }
    common::cleanup(&dir);
}
