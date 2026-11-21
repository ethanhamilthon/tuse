use crate::app::App;
use crate::chat::{Chat, Entry};
use crate::chat_anchor::Slot;

/// What `Chat::set_print` did; `Removed` carries the index of a removed entry.
#[derive(Debug, PartialEq)]
pub enum PrintChange {
    Added,
    Updated,
    Removed(Option<usize>),
}

impl Chat {
    /// Sets the print block `id`: replaces its text, or adds it (`pin` only matters
    /// then). Empty `text` on an existing block removes it.
    pub fn set_print(&mut self, id: &str, text: String, pin: bool) -> PrintChange {
        let is = |e: &Entry| matches!(e, Entry::Print { id: x, .. } if x == id);
        let slot = match self.pins.iter().position(|(e, _)| is(e)) {
            Some(i) => Some(Slot::Pin(i)),
            None => self
                .entries
                .iter()
                .rposition(|(e, _)| is(e))
                .map(Slot::Entry),
        };
        if let Some(slot) = slot {
            self.record_anchor(slot);
            if text.is_empty() {
                return match slot {
                    Slot::Pin(i) => {
                        self.pins.remove(i);
                        self.shift_anchor(slot);
                        PrintChange::Removed(None)
                    }
                    Slot::Entry(i) => {
                        self.remove(i);
                        self.shift_anchor(slot);
                        PrintChange::Removed(Some(i))
                    }
                };
            }
            let target = match slot {
                Slot::Pin(i) => self.pins.get_mut(i),
                Slot::Entry(i) => self.entries.get_mut(i),
            };
            if let Some((Entry::Print { text: old, .. }, cache)) = target {
                *old = text;
                *cache = None;
            }
            return PrintChange::Updated;
        }
        if text.is_empty() {
            return PrintChange::Removed(None);
        }
        let e = Entry::Print {
            id: id.to_string(),
            text,
        };
        if pin {
            self.pins.push((e, None));
        } else {
            self.push(e);
        }
        PrintChange::Added
    }
}

impl App {
    /// Fixes the stored entry indexes after the entry at `pos` was removed.
    pub(crate) fn shift_after(&mut self, pos: usize) {
        for idx in [&mut self.streaming, &mut self.thinking]
            .into_iter()
            .flatten()
        {
            if *idx > pos {
                *idx -= 1;
            }
        }
        for (_, at) in &mut self.pending_tools {
            if *at > pos {
                *at -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::Chat;
    use crate::style::Palette;

    fn texts(c: &mut Chat, scroll: usize) -> (String, usize) {
        let p = Palette::new(&Default::default(), &Default::default());
        let (lines, s) = c.window(20, &p, scroll, 50);
        let all = lines
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
            .filter(|t| !t.trim().is_empty())
            .collect::<Vec<_>>();
        (all.join(","), s)
    }

    #[test]
    fn print_blocks_are_replaced_in_place() {
        let mut c = Chat::default();
        assert_eq!(c.set_print("p1", "a".into(), false), PrintChange::Added);
        c.push(Entry::Notice("n".into()));
        assert_eq!(c.set_print("p1", "b".into(), false), PrintChange::Updated);
        assert_eq!(c.len(), 2);
        assert!(matches!(c.get_mut(0), Some(Entry::Print { text, .. }) if text == "b"));
        assert_eq!(
            c.set_print("p1", "".into(), false),
            PrintChange::Removed(Some(0))
        );
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn pinned_blocks_stay_last_in_creation_order() {
        let mut c = Chat::default();
        c.set_print("a", "pin-a".into(), true);
        c.push(Entry::Notice("one".into()));
        c.set_print("b", "pin-b".into(), true);
        c.push(Entry::Notice("two".into()));
        c.set_print("a", "pin-a2".into(), true);
        assert_eq!(texts(&mut c, 0).0, "✓ one,✓ two,pin-a2,pin-b");
        assert_eq!(
            c.set_print("a", "".into(), true),
            PrintChange::Removed(None)
        );
        assert_eq!(texts(&mut c, 0).0, "✓ one,✓ two,pin-b");
        // The id is free again.
        c.set_print("a", "new".into(), false);
        assert_eq!(texts(&mut c, 0).0, "✓ one,✓ two,new,pin-b");
    }

    #[test]
    fn pin_updates_refresh_the_cache_and_keep_the_view_still() {
        let mut c = Chat::default();
        for i in 0..100 {
            c.push(Entry::Notice(format!("n{i}")));
        }
        c.set_print("a", "x".into(), true);
        texts(&mut c, 0);
        let (_, s) = texts(&mut c, 5);
        assert_eq!(s, 5);
        let before = texts(&mut c, 5).0;
        c.set_print("a", "x\ny\nz".into(), true);
        let (after, s) = texts(&mut c, 5);
        assert_eq!(s, 7);
        assert_eq!(before, after);
        // Removing the pin leaves the view in range.
        c.set_print("a", "".into(), true);
        let (_, s) = texts(&mut c, s);
        assert!(s < 7);
    }

    #[test]
    fn unpinned_redraw_below_the_viewport_keeps_the_view_still() {
        let p = Palette::new(&Default::default(), &Default::default());
        let mut c = Chat::default();
        for i in 0..60 {
            c.push(Entry::Notice(format!("n{i}")));
        }
        c.set_print("a", "x".into(), false);
        for i in 0..20 {
            c.push(Entry::Notice(format!("m{i}")));
        }
        let view = |c: &mut Chat, s| {
            let (l, s) = c.window(20, &p, s, 10);
            let t: Vec<String> = l
                .iter()
                .map(|l| l.iter().map(|s| s.text.as_str()).collect())
                .collect();
            (t, s)
        };
        let (before, s) = view(&mut c, 60);
        c.set_print("a", "x\ny\nz\nw".into(), false);
        let (after, s2) = view(&mut c, s);
        assert_eq!(before, after);
        assert!(s2 > s);
        c.set_print("a", "".into(), false);
        let (gone, _) = view(&mut c, s2);
        assert_eq!(before, gone);
    }
}
