use super::*;
use crate::store::SessionDir;

fn session(id: &str, parent: &str, tokens: u64, model: &str) {
    let mut d = SessionDir::open(id, true).unwrap();
    d.append(&Record::User {
        text: "hi".into(),
        context: String::new(),
        images: vec![],
        at: String::new(),
    });
    d.append(&Record::Usage {
        model: model.into(),
        usage: Usage {
            input: tokens,
            output: 10,
            ..Default::default()
        },
    });
    d.write_meta(&Meta {
        id: id.into(),
        parent: parent.into(),
        created: "2026-11-19T00:00:00Z".into(),
        updated: "2026-11-19T00:01:00Z".into(),
        ..Default::default()
    });
    if id == "live" {
        std::mem::forget(d);
    }
}

#[test]
fn tree_totals_status_and_torn_lines() {
    let home = std::env::temp_dir().join(format!("tuse-children-{}", std::process::id()));
    // SAFETY: this is the only test in the crate's unit tests that sets `TUSE_HOME`.
    unsafe { std::env::set_var("TUSE_HOME", &home) };
    let mut config = Config::default();
    config.models.push(tuse_config::ModelDef {
        name: "p/m".into(),
        window: 1000,
        prices: Some(tuse_config::Prices {
            input: 1_000_000.0,
            ..Default::default()
        }),
        ..Default::default()
    });
    session("a", "root", 100, "p/m");
    session("b", "a", 200, "p/m");
    session("live", "a", 5, "other");
    session("other", "unrelated", 1, "p/m");
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(home.join("b/session.jsonl"))
        .unwrap();
    std::io::Write::write_all(&mut log, b"{\"t\":\"usage\",\"mo").unwrap();

    let list = descendants("root", &config);
    let ids: Vec<_> = list.iter().map(|c| (c.id.as_str(), c.depth)).collect();
    assert_eq!(ids, [("a", 0), ("b", 1), ("live", 1)]);
    assert!(list[0].has_children && !list[1].has_children);
    assert!(!list[1].running && list[2].running);
    assert_eq!(list[1].elapsed_secs, 60);
    assert_eq!((list[1].context, list[1].window), (210, Some(1000)));
    assert_eq!(list[2].window, None);
    assert_eq!(list[2].own.cost, None);
    assert_eq!(list[0].own.usage.input, 100);
    assert_eq!(list[0].subtree.usage.input, 305);
    assert_eq!(list[0].own.cost, Some(100.0));
    assert_eq!(list[0].subtree.cost, Some(300.0));
    assert!(descendants("nobody", &config).is_empty());
    std::fs::remove_dir_all(home).ok();
}
