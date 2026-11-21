//! Keeps the view still when a `draw_ui` block changes height while the user is scrolled up.
use crate::chat::Chat;
use crate::style::Palette;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Slot {
    Entry(usize),
    Pin(usize),
}

/// Where the last `window` call put the entries: slot, row (from the bottom) of its last
/// line, and its height.
#[derive(Default)]
pub(crate) struct Layout {
    pub segs: Vec<(Slot, usize, usize)>,
    pub scroll: usize,
    pub height: usize,
}

/// A visible row outside the changing block: its entry, its line in it, and how many
/// rows above the bottom of the screen it was drawn.
pub(crate) struct Anchor {
    slot: Slot,
    row: usize,
    q: usize,
}

impl Chat {
    pub(crate) fn slot_at(&self, i: usize) -> Slot {
        match i.checked_sub(self.entries.len()) {
            Some(j) => Slot::Pin(j),
            None => Slot::Entry(i),
        }
    }

    /// Call before `block` changes height: remembers the topmost visible row that is not
    /// part of it (or the top row when the viewport is all inside it). No-op at the bottom.
    pub(crate) fn record_anchor(&mut self, block: Slot) {
        let l = &self.layout;
        if self.pending.is_some() || l.scroll == 0 {
            return;
        }
        let find = |r: usize| {
            l.segs
                .iter()
                .find(|(_, start, h)| (*start..start + h).contains(&r))
                .map(|(slot, start, h)| (*slot, h - 1 - (r - start), r - l.scroll))
        };
        let rows = (l.scroll..l.scroll + l.height).rev();
        let found = rows
            .clone()
            .filter_map(find)
            .find(|(s, ..)| *s != block)
            .or_else(|| rows.filter_map(find).next());
        self.pending = found.map(|(slot, row, q)| Anchor { slot, row, q });
    }

    /// Call after the entry/pin `removed` was taken out: later slots move up by one.
    pub(crate) fn shift_anchor(&mut self, removed: Slot) {
        let Some(a) = &mut self.pending else { return };
        match (&mut a.slot, removed) {
            (s, r) if *s == r => self.pending = None,
            (Slot::Entry(j), Slot::Entry(i)) | (Slot::Pin(j), Slot::Pin(i)) if *j > i => *j -= 1,
            _ => {}
        }
    }

    /// The scroll that keeps the recorded anchor row where it was on screen.
    pub(crate) fn apply_anchor(&mut self, width: usize, p: &Palette, scroll: usize) -> usize {
        let Some(a) = self.pending.take() else {
            return scroll;
        };
        if scroll == 0 {
            return 0;
        }
        let gap = p.ui.message_gap as usize;
        let target = match a.slot {
            Slot::Entry(i) => i,
            Slot::Pin(i) => self.entries.len() + i,
        };
        let mut below = 0;
        for i in (0..self.entries.len() + self.pins.len()).rev() {
            let h = self.lines(i, width, p).len();
            if h == 0 {
                continue;
            }
            let g = if below == 0 { 0 } else { gap };
            if i == target {
                return (below + g + (h - 1 - a.row.min(h - 1))).saturating_sub(a.q);
            }
            below += g + h;
        }
        scroll
    }
}

#[cfg(test)]
#[path = "chat_anchor_tests.rs"]
mod tests;
