#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Command,
    Prompt,
    File,
    Resume,
    Model,
    Effort(String),
    Task,
    Children,
    Trace,
}

impl Kind {
    /// Token pickers filter by the token under the input cursor; the others
    /// are modal and take over the editor as their search box.
    pub fn is_token(&self) -> bool {
        matches!(self, Kind::Command | Kind::Prompt | Kind::File)
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub value: String,
    /// A short column before the label (the role in `/trace`); it is searched too.
    pub tag: String,
}

pub fn item(label: impl Into<String>, detail: impl Into<String>, value: impl Into<String>) -> Item {
    Item {
        label: label.into(),
        detail: detail.into(),
        value: value.into(),
        tag: String::new(),
    }
}

pub fn tagged(tag: &str, label: impl Into<String>, value: impl Into<String>) -> Item {
    Item {
        tag: tag.into(),
        ..item(label, "", value)
    }
}

pub struct Picker {
    pub kind: Kind,
    pub title: String,
    pub items: Vec<Item>,
    pub shown: Vec<usize>,
    pub selected: usize,
    pub scroll: usize,
}

impl Picker {
    pub fn new(kind: Kind, title: impl Into<String>, items: Vec<Item>) -> Self {
        let mut p = Picker {
            kind,
            title: title.into(),
            shown: (0..items.len()).collect(),
            items,
            selected: 0,
            scroll: 0,
        };
        p.filter("");
        p
    }

    pub fn filter(&mut self, q: &str) {
        let trace = (self.kind == Kind::Trace).then(|| crate::trace_filter::parse(q));
        let words = trace.as_ref().map_or(q, |t| t.text.as_str());
        let mut scored: Vec<(i64, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| trace.as_ref().is_none_or(|t| t.allows(&it.tag)))
            .filter_map(|(i, it)| {
                let text = if trace.is_some() {
                    it.label.clone()
                } else {
                    format!("{} {}", it.tag, it.label)
                };
                score(&text, words).map(|s| (s, i))
            })
            .collect();
        // The trace keeps its order; other lists rank by score.
        if !words.is_empty() && trace.is_none() {
            scored.sort_by_key(|(s, i)| (-*s, *i));
        }
        self.shown = scored.into_iter().map(|(_, i)| i).collect();
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn current(&self) -> Option<&Item> {
        self.shown.get(self.selected).map(|&i| &self.items[i])
    }

    pub fn select(&mut self, value: &str) {
        if let Some(pos) = self
            .shown
            .iter()
            .position(|&i| self.items[i].value == value)
        {
            self.selected = pos;
        }
    }

    /// Scrolls so that the selected row is among the `height` visible ones.
    pub fn follow(&mut self, height: usize) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }
    }

    pub fn moved(&mut self, delta: isize, height: usize) {
        let n = self.shown.len();
        if n == 0 {
            return;
        }
        let next = self.selected as isize + delta;
        self.selected = if self.kind == Kind::Trace {
            next.clamp(0, (n - 1) as isize) as usize
        } else {
            next.rem_euclid(n as isize) as usize
        };
        self.follow(height);
    }
}

/// Fuzzy subsequence score; substring and prefix matches rank first.
pub fn score(label: &str, q: &str) -> Option<i64> {
    if q.is_empty() {
        return Some(0);
    }
    let l = label.to_lowercase();
    let q = q.to_lowercase();
    if let Some(pos) = l.find(&q) {
        let base = if pos == 0 { 3000 } else { 2000 };
        let boundary = pos > 0 && matches!(l.as_bytes()[pos - 1], b'/' | b'-' | b'_' | b'.' | b' ');
        return Some(base + i64::from(boundary) * 500 - pos as i64 - l.len() as i64);
    }
    let mut chars = l.chars();
    let mut gaps = 0;
    for qc in q.chars() {
        let mut skipped = 0;
        loop {
            match chars.next() {
                Some(c) if c == qc => break,
                Some(_) => skipped += 1,
                None => return None,
            }
        }
        gaps += skipped;
    }
    Some(1000 - gaps - l.len() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_ranks_substrings_first() {
        let items = [
            "src/main.rs",
            "crates/tui/src/app.rs",
            "README.md",
            "mars.txt",
        ]
        .map(|s| item(s, "", s))
        .to_vec();
        let mut p = Picker::new(Kind::File, "", items);
        p.filter("mars");
        assert_eq!(p.current().unwrap().label, "mars.txt");
        assert_eq!(p.shown.len(), 2);
        p.filter("zzz");
        assert!(p.current().is_none());
    }

    #[test]
    fn trace_picker_clamps_instead_of_wrapping() {
        let items = ["a", "b", "c"].map(|s| item(s, "", s)).to_vec();
        let mut trace = Picker::new(Kind::Trace, "Trace", items.clone());
        trace.moved(-1, 5);
        assert_eq!(trace.selected, 0);
        trace.moved(10, 5);
        assert_eq!(trace.selected, 2);
        trace.moved(1, 5);
        assert_eq!(trace.selected, 2);

        let mut other = Picker::new(Kind::Command, "", items);
        other.moved(-1, 5);
        assert_eq!(other.selected, 2);
    }
}
