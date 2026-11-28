#[derive(Debug, Clone, PartialEq)]
pub enum ManagePick {
    ProviderSub,
    PluginSub,
    ConnectApi,
    ProviderUpdate,
    UpdateApi { name: String, current_base: String },
    ProviderDisconnect,
    PluginGet,
    PluginDelete,
    PluginUpdate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Command,
    Prompt,
    CommandArg(String),
    File,
    Resume,
    Fork,
    Model,
    Effort(String),
    Task,
    History,
    LuaPicker(u64),
    Manage(ManagePick),
}

impl Kind {
    pub fn is_token(&self) -> bool {
        matches!(
            self,
            Kind::Command | Kind::CommandArg(_) | Kind::Prompt | Kind::File
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

    fn match_key(&self, kind: &Kind) -> &str {
        match kind {
            Kind::File => self.label.trim_end_matches('/'),
            _ => &self.value,
        }
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
    pub title: Option<String>,
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
            title: None,
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
            .filter_map(|(i, item)| {
                let matches = if query.is_empty() {
                    true
                } else if self.kind.is_token() {
                    let key = item.match_key(&self.kind).to_lowercase();
                    if self.kind == Kind::File {
                        key.contains(&query)
                    } else {
                        key.starts_with(&query)
                    }
                } else {
                    item.cells().join(" ").to_lowercase().contains(&query)
                };
                matches.then_some(i)
            })
            .collect();
        if self.kind == Kind::File && !query.is_empty() {
            self.shown.sort_by_cached_key(|&i| {
                !self.items[i]
                    .match_key(&self.kind)
                    .to_lowercase()
                    .starts_with(&query)
            });
        }
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

    pub fn page(&mut self, delta: isize, height: usize) {
        let n = self.shown.len();
        if n == 0 {
            return;
        }
        let next = (self.selected as isize + delta).clamp(0, (n - 1) as isize);
        self.selected = next as usize;
        self.follow(height);
    }
}

pub const PROVIDERS_ARGS: [(&str, &str); 4] = [
    ("list", "list saved providers"),
    ("connect", "connect a provider"),
    ("update", "edit a saved provider"),
    ("disconnect", "remove a saved provider"),
];

pub const PLUGINS_ARGS: [(&str, &str); 5] = [
    ("list", "list installed plugins"),
    ("get", "inspect an installed plugin"),
    ("add", "install a plugin from source"),
    ("delete", "remove an installed plugin"),
    ("update", "update installed plugins"),
];

pub const COMMANDS: [(&str, &str); 13] = [
    ("config", "show the resolved config"),
    ("new", "start a new session"),
    ("resume", "open a session from this folder"),
    ("fork", "branch the session at a message"),
    ("models", "switch model and effort"),
    ("providers", "manage provider profiles"),
    ("plugins", "manage installed plugins"),
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
    fn token_pickers_match_name_prefixes_only() {
        let names = ["imagegen", "openspec", "release", "srt", "use-figma"];
        let mut prompts = Picker::new(
            Kind::Prompt,
            names.map(|n| item(format!("#{n}"), "release", n)).into(),
        );
        for q in ["r", "REL"] {
            prompts.filter(q);
            assert_eq!(prompts.shown, [2]);
        }
        let mut commands = Picker::new(
            Kind::Command,
            COMMANDS.map(|(n, d)| item(format!("/{n}"), d, n)).into(),
        );
        commands.filter("r");
        assert_eq!(
            commands
                .shown
                .iter()
                .map(|&i| commands.items[i].value.as_str())
                .collect::<Vec<_>>(),
            ["resume", "reload"]
        );
        let mut args = Picker::new(
            Kind::CommandArg("plugins".into()),
            ["list", "get", "add"].map(|a| item(a, "", a)).into(),
        );
        args.filter("GE");
        assert_eq!(args.shown, [1]);
    }

    #[test]
    fn file_picker_groups_prefixes_first_without_changing_recency_within_groups() {
        let items = ["domain.rs", "map.rs", "roman.rs", "main.rs", "mod.rs"];
        let mut picker = Picker::new(
            Kind::File,
            items.map(|n| item(n, "", format!("ma/{n}"))).into(),
        );
        picker.filter("");
        assert_eq!(picker.shown, [0, 1, 2, 3, 4]);
        picker.filter("MA");
        assert_eq!(picker.shown, [1, 3, 0, 2]);
    }

    #[test]
    fn modal_picker_searches_detail_and_preserves_column_widths() {
        let items = vec![item("a", "needle here", "a"), item("bb", "", "b")];
        let mut picker = Picker::new(Kind::Resume, items);
        let widths = picker.widths.clone();
        picker.filter("needle");
        assert_eq!(picker.shown, [0]);
        assert_eq!(picker.widths, widths);
    }
}
