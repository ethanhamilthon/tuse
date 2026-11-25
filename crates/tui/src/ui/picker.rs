#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Command,
    Prompt,
    PromptArg(String),
    CommandArg(String),
    File,
    Resume,
    Model,
    Effort(String),
    Task,
    History,
    LuaPicker(u64),
}

impl Kind {
    pub fn is_token(&self) -> bool {
        matches!(
            self,
            Kind::Command | Kind::CommandArg(_) | Kind::Prompt | Kind::PromptArg(_) | Kind::File
        )
    }

    pub fn is_searchable(&self) -> bool {
        !self.is_token() && !matches!(self, Kind::LuaPicker(_))
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub lead: Vec<String>,
    pub label: String,
    pub detail: String,
    pub value: String,
}

impl Item {
    pub fn lead(mut self, cell: impl Into<String>) -> Self {
        self.lead.push(cell.into());
        self
    }

    pub fn cells(&self) -> Vec<String> {
        self.lead
            .iter()
            .chain([&self.label, &self.detail])
            .map(|c| c.replace('\n', " "))
            .collect()
    }
}

pub fn item(label: impl Into<String>, detail: impl Into<String>, value: impl Into<String>) -> Item {
    Item {
        lead: vec![],
        label: label.into(),
        detail: detail.into(),
        value: value.into(),
    }
}

pub struct Picker {
    pub kind: Kind,
    pub items: Vec<Item>,
    pub shown: Vec<usize>,
    pub selected: usize,
    pub scroll: usize,
    pub widths: Vec<usize>,
}

impl Picker {
    pub fn new(kind: Kind, items: Vec<Item>) -> Self {
        let shown = (0..items.len()).collect();
        let mut widths: Vec<usize> = vec![];
        for it in &items {
            for (c, cell) in it.cells().iter().enumerate() {
                if widths.len() <= c {
                    widths.resize(c + 1, 0);
                }
                widths[c] = widths[c].max(crate::kit::text::width(cell));
            }
        }
        Picker {
            kind,
            widths,
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
            .filter(|(_, item)| {
                query.is_empty() || item.cells().join(" ").to_lowercase().contains(&query)
            })
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

pub const COMMANDS: [(&str, &str); 10] = [
    ("config", "show the resolved config"),
    ("new", "start a new session"),
    ("resume", "open a session from this folder"),
    ("models", "switch model and effort"),
    ("tasks", "background tasks"),
    ("session", "session info, usage and model"),
    ("history", "everything in this session"),
    ("reload", "run the config file again"),
    ("stop", "stop the turn, tasks, hooks and commands"),
    ("quit", "exit"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_by_case_insensitive_substring_in_list_order() {
        let items = ["Mars One", "Gallery", "mars Two"].map(|s| item(s, "", s));
        let mut picker = Picker::new(Kind::File, items.into());
        picker.filter("mArS");
        assert_eq!(picker.shown, [0, 2]);
        picker.filter("m One");
        assert!(picker.shown.is_empty());

        let items = vec![item("a", "needle here", "a"), item("bb", "", "b")];
        let mut picker = Picker::new(Kind::Resume, items);
        let widths = picker.widths.clone();
        picker.filter("needle");
        assert_eq!(picker.shown, [0]);
        assert_eq!(picker.widths, widths);
    }
}
