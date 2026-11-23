#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Command,
    Prompt,
    File,
    Resume,
    Model,
    Effort(String),
    Task,
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
}

pub fn item(label: impl Into<String>, detail: impl Into<String>, value: impl Into<String>) -> Item {
    Item {
        label: label.into(),
        detail: detail.into(),
        value: value.into(),
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
        let shown = (0..items.len()).collect();
        Picker {
            kind,
            title: title.into(),
            items,
            shown,
            selected: 0,
            scroll: 0,
        }
    }

    pub fn filter(&mut self, query: &str) {
        let query = query.to_lowercase();
        self.shown = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| query.is_empty() || item.label.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
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
        self.selected = next.rem_euclid(n as isize) as usize;
        self.follow(height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_by_case_insensitive_substring_in_list_order() {
        let items = ["Mars One", "Gallery", "mars Two"].map(|s| item(s, "", s));
        let mut picker = Picker::new(Kind::File, "", items.into());
        picker.filter("mArS");
        assert_eq!(picker.shown, [0, 2]);
        picker.filter("m One");
        assert!(picker.shown.is_empty());
    }
}
