use super::*;
#[test]
fn notifications_are_not_user_entries() {
    let c = Chat::from_records(
        &[Record::Notification {
            text: "ready".into(),
        }],
        &str::to_owned,
    );
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

#[test]
#[ignore]
fn bench_stream() {
    let p = Palette::new(&Default::default(), &Default::default());
    let block = "## Section\n\nSome **bold** text with `code` and a [link](x). More words here to wrap around.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n- one\n- two\n\n";
    let full = block.repeat(30000 / block.len() + 1);
    let mut c = Chat::default();
    c.push(Entry::Assistant(String::new()));
    let start = std::time::Instant::now();
    let mut frames = 0;
    for chunk in full.as_bytes().chunks(40) {
        if let Some(Entry::Assistant(s)) = c.get_mut(0) {
            s.push_str(std::str::from_utf8(chunk).unwrap());
        }
        c.window(100, &p, 0, 50);
        frames += 1;
    }
    eprintln!(
        "BENCH {} bytes, {frames} frames, {:?}",
        full.len(),
        start.elapsed()
    );
}
