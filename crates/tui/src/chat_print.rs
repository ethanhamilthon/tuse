use crate::app::App;
use crate::chat::{Chat, Entry};
use crate::style::Palette;

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
        if let Some(i) = self.pins.iter().position(|(e, _)| is(e)) {
            if text.is_empty() {
                self.pins.remove(i);
                return PrintChange::Removed(None);
            }
            if let Entry::Print { text: old, .. } = &mut self.pins[i].0 {
                *old = text;
                self.pins[i].1 = None;
            }
            return PrintChange::Updated;
        }
        if let Some(i) = self.entries.iter().rposition(|(e, _)| is(e)) {
            if text.is_empty() {
                self.remove(i);
                return PrintChange::Removed(Some(i));
            }
            if let Some(Entry::Print { text: old, .. }) = self.get_mut(i) {
                *old = text;
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

    /// Row cost (lines plus gap) of the print block `id`; 0 when it is gone or empty.
    fn print_cost(&mut self, id: &str, width: usize, p: &Palette) -> usize {
        let n = self.entries.len();
        let is = |e: &Entry| matches!(e, Entry::Print { id: x, .. } if x == id);
        let at = match self.pins.iter().position(|(e, _)| is(e)) {
            Some(i) => n + i,
            None => match self.entries.iter().rposition(|(e, _)| is(e)) {
                Some(i) => i,
                None => return 0,
            },
        };
        match self.lines(at, width, p).len() {
            0 => 0,
            h => h + p.ui.message_gap as usize,
        }
    }

    /// Moves `scroll` by the size changes of the print blocks below the viewport and
    /// remembers which blocks are below it now.
    pub(crate) fn anchor(&mut self, width: usize, p: &Palette, scroll: usize) -> usize {
        let mut delta = 0isize;
        for (id, old) in std::mem::take(&mut self.below) {
            delta += self.print_cost(&id, width, p) as isize - old as isize;
        }
        let scroll = if scroll == 0 {
            0
        } else {
            scroll.saturating_add_signed(delta)
        };
        let gap = p.ui.message_gap as usize;
        // Rows from the bottom to the top of the entry's text, as `window` lays them out:
        // the gap sits below every entry but the last.
        let mut end = 0;
        for i in (0..self.entries.len() + self.pins.len()).rev() {
            let (is_print, id) = match &self.slot(i).0 {
                Entry::Print { id, .. } => (true, id.clone()),
                _ => (false, String::new()),
            };
            let h = self.lines(i, width, p).len();
            if h == 0 {
                continue;
            }
            end += if end == 0 { h } else { h + gap };
            if end > scroll || scroll == 0 {
                break;
            }
            if is_print {
                self.below.push((id, h + gap));
            }
        }
        scroll
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

    #[test]
    fn redraw_of_a_block_just_below_the_viewport_keeps_the_view_still() {
        let p = Palette::new(&Default::default(), &Default::default());
        let mut c = Chat::default();
        for i in 0..30 {
            c.push(Entry::Notice(format!("n{i}")));
        }
        c.set_print("a", "x".into(), true);
        let view = |c: &mut Chat, s| {
            let (l, s) = c.window(20, &p, s, 10);
            let t: Vec<String> = l
                .iter()
                .map(|l| l.iter().map(|s| s.text.as_str()).collect())
                .collect();
            (t, s)
        };
        // Scroll 1 starts in the gap right above the pin's text.
        let (before, s) = view(&mut c, 1);
        c.set_print("a", "x\ny\nz".into(), true);
        let (after, s) = view(&mut c, s);
        assert_eq!(before, after);
        assert_eq!(s, 3);
    }
}
