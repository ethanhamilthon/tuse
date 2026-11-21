mod common;
use tuse_core::{Event, Mode, Options, Session};

#[test]
fn redraws_report_the_creation_pin_and_removal_frees_the_id() {
    let addr = common::provider(vec![]);
    let (dir, cfg) = common::setup("print-pin", &addr, "");
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Tui,
        resume: None,
        start: Default::default(),
    })
    .unwrap();
    let events = s.subscribe();
    let id = s.print("a".into(), None, true).unwrap();
    s.print("b".into(), Some(id.clone()), false).unwrap();
    s.print(String::new(), Some(id.clone()), false).unwrap();
    assert!(s.print("c".into(), Some(id.clone()), false).is_err());
    let again = s.print("d".into(), None, false).unwrap();
    assert_eq!(again, id);
    let got: Vec<(String, bool)> = events
        .try_iter()
        .filter_map(|e| match e {
            Event::Print { text, pin, .. } => Some((text, pin)),
            _ => None,
        })
        .collect();
    let want = [("a", true), ("b", true), ("", true), ("d", false)];
    assert_eq!(got, want.map(|(t, p)| (t.to_string(), p)));
    s.close();
    common::cleanup(&dir);
}
