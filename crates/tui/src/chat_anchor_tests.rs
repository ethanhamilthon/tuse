use crate::chat::{Chat, Entry};
use crate::style::Palette;

const H: usize = 10;

fn view(c: &mut Chat, scroll: usize) -> (Vec<String>, usize) {
    let p = Palette::new(&Default::default(), &Default::default());
    let (lines, s) = c.window(20, &p, scroll, H);
    let t = lines
        .iter()
        .map(|l| l.iter().map(|s| s.text.as_str()).collect())
        .collect();
    (t, s)
}

/// A chat with 40 notices, `a` and `b` around the print block, and 40 more notices.
fn chat(pin: bool) -> Chat {
    let mut c = Chat::default();
    for i in 0..40 {
        c.push(Entry::Notice(format!("a{i}")));
    }
    c.set_print("x", "1\n2".into(), pin);
    for i in 0..40 {
        c.push(Entry::Notice(format!("b{i}")));
    }
    c
}

/// Rows of the notices that stay visible must keep their screen position.
fn same_notices(before: &[String], after: &[String]) {
    let pick = |v: &[String]| -> Vec<(usize, String)> {
        v.iter()
            .enumerate()
            .filter(|(_, t)| t.contains('a') || t.contains('b'))
            .map(|(i, t)| (i, t.clone()))
            .collect()
    };
    let (b, a) = (pick(before), pick(after));
    assert!(!b.is_empty());
    assert!(b.iter().all(|r| a.contains(r)) || a.iter().all(|r| b.contains(r)));
}

fn redraw_at(pin: bool, scroll: usize, text: &str) {
    let mut c = chat(pin);
    view(&mut c, 0);
    let (before, s) = view(&mut c, scroll);
    assert_eq!(s, scroll);
    c.set_print("x", text.into(), pin);
    let (after, _) = view(&mut c, s);
    same_notices(&before, &after);
}

fn offsets(pin: bool) -> [usize; 4] {
    // Pinned: the block is the last entry (text rows 0-1), else it sits 40 notices up.
    if pin {
        [1, 2, 3, 12]
    } else {
        [82, 83, 85, 100]
    }
}

const BIG: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13";

#[test]
fn redraws_keep_the_view_for_every_block_position() {
    for pin in [true, false] {
        // Viewport starts inside the block, in the gap above it, and with the block below.
        for s in offsets(pin) {
            redraw_at(pin, s, BIG);
            redraw_at(pin, s, "1");
        }
    }
}

#[test]
fn block_above_the_viewport_keeps_the_view() {
    let mut c = chat(false);
    view(&mut c, 0);
    let (before, s) = view(&mut c, 10);
    c.set_print("x", BIG.into(), false);
    let (after, _) = view(&mut c, s);
    assert_eq!(before, after);
}

#[test]
fn removal_keeps_the_view_and_scroll_zero_follows_the_bottom() {
    for pin in [true, false] {
        for s in offsets(pin) {
            let mut c = chat(pin);
            view(&mut c, 0);
            let (before, s) = view(&mut c, s);
            c.set_print("x", String::new(), pin);
            let (after, now) = view(&mut c, s);
            // Content that ended below the anchor cannot be held: the view settles at the bottom.
            if now > 0 {
                same_notices(&before, &after);
            } else {
                assert_eq!(after.last().map(String::as_str), Some("✓ b39"));
            }
        }
    }
    let mut c = chat(true);
    view(&mut c, 0);
    c.set_print("x", BIG.into(), true);
    let (at_bottom, s) = view(&mut c, 0);
    assert_eq!(s, 0);
    assert_eq!(at_bottom.last().map(String::as_str), Some("13"));
}
