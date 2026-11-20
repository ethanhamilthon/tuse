use super::*;
#[test]
fn notifications_are_not_user_entries() {
    let c = Chat::from_records(&[Record::Notification {
        text: "ready".into(),
    }]);
    assert!(matches!(&c.entries[0].0, Entry::Notice(t) if t == "ready"));
}

#[test]
fn print_blocks_are_replaced_in_place() {
    let mut c = Chat::default();
    assert!(c.set_print("p1", "a".into()));
    c.push(Entry::Notice("n".into()));
    assert!(!c.set_print("p1", "b".into()));
    assert_eq!(c.len(), 2);
    assert!(matches!(c.get_mut(0), Some(Entry::Print { text, .. }) if text == "b"));
}

#[test]
fn window_slices_and_invalidate_clears_cache() {
    let p = Palette::new(&Default::default(), &Default::default());
    let mut c = Chat::default();
    c.push(Entry::Notice("one".into()));
    c.push(Entry::Notice("two".into()));
    c.push(Entry::Notice("three".into()));
    let (rows, scroll) = c.window(40, &p, 0, 2);
    assert_eq!(scroll, 0);
    assert_eq!(rows.len(), 2);
    c.invalidate();
    assert!(c.entries.iter().all(|(_, cache)| cache.is_none()));
}
