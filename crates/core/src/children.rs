use crate::record::Record;
use crate::store::{Meta, is_locked, read_meta, read_records, tuse_home};
use crate::usage::cost;
use std::collections::{HashMap, HashSet};
use tuse_api::Usage;
use tuse_config::Config;

/// Tokens and spend; `cost` is `None` when no model involved has a price.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Totals {
    pub usage: Usage,
    pub cost: Option<f64>,
}

impl Totals {
    fn add(&mut self, o: &Totals) {
        self.usage.add(&o.usage);
        self.cost = match (self.cost, o.cost) {
            (None, c) | (c, None) => c,
            (Some(a), Some(b)) => Some(a + b),
        };
    }
}

/// One descendant session.
#[derive(Debug, Clone, Default)]
pub struct Child {
    pub id: String,
    /// The title, else a short id.
    pub name: String,
    pub depth: usize,
    pub running: bool,
    pub cwd: String,
    pub elapsed_secs: u64,
    pub own: Totals,
    /// Own totals plus those of every descendant.
    pub subtree: Totals,
    pub has_children: bool,
    /// Input plus output of the last request.
    pub context: u64,
    /// The model's context window, when the config knows it.
    pub window: Option<u64>,
}

fn short(id: &str) -> String {
    id.rsplit('-')
        .next()
        .unwrap_or(id)
        .chars()
        .take(8)
        .collect()
}

fn own(records: &[Record], config: &Config) -> (Totals, u64, Option<u64>) {
    let mut t = Totals::default();
    let (mut context, mut window) = (0, None);
    for r in records {
        let Record::Usage { model, usage } = r else {
            continue;
        };
        t.usage.add(usage);
        let def = config.model_def(model);
        if let Some(p) = def.and_then(|d| d.prices.as_ref()) {
            t.cost = Some(t.cost.unwrap_or(0.0) + cost(usage, p));
        }
        context = usage.input + usage.output;
        window = def.map(|d| d.window).filter(|w| *w > 0);
    }
    (t, context, window)
}

fn node(m: &Meta, depth: usize, now: u64, config: &Config) -> Child {
    let dir = tuse_home().join(&m.id);
    let running = is_locked(&dir);
    let (own, context, window) = own(&read_records(&dir), config);
    let start = crate::id::parse(&m.created).unwrap_or(now);
    let end = if running {
        now
    } else {
        crate::id::parse(&m.updated).unwrap_or(now)
    };
    Child {
        id: m.id.clone(),
        name: if m.title.is_empty() {
            short(&m.id)
        } else {
            m.title.clone()
        },
        depth,
        running,
        cwd: m.cwd.clone(),
        elapsed_secs: end.saturating_sub(start),
        own,
        subtree: own,
        has_children: false,
        context,
        window,
    }
}

impl crate::session::Session {
    /// Every descendant of this session; see `descendants`.
    pub fn children(&self) -> Vec<Child> {
        descendants(&self.id, &self.config())
    }
}

/// Every descendant of `root` at any depth, parents before their children
/// (siblings oldest first), running or finished.
pub fn descendants(root: &str, config: &Config) -> Vec<Child> {
    let mut by_parent: HashMap<String, Vec<Meta>> = HashMap::new();
    for e in std::fs::read_dir(tuse_home())
        .into_iter()
        .flatten()
        .flatten()
    {
        if let Some(m) = read_meta(&e.path()).filter(|m| !m.parent.is_empty() && m.id != root) {
            by_parent.entry(m.parent.clone()).or_default().push(m);
        }
    }
    for v in by_parent.values_mut() {
        v.sort_by(|a, b| a.created.cmp(&b.created).then(a.id.cmp(&b.id)));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (mut out, mut seen) = (vec![], HashSet::new());
    walk(root, 0, &by_parent, now, config, &mut seen, &mut out);
    out
}

fn walk(
    parent: &str,
    depth: usize,
    by_parent: &HashMap<String, Vec<Meta>>,
    now: u64,
    config: &Config,
    seen: &mut HashSet<String>,
    out: &mut Vec<Child>,
) -> Totals {
    let mut sum = Totals::default();
    for m in by_parent.get(parent).into_iter().flatten() {
        if !seen.insert(m.id.clone()) {
            continue;
        }
        let at = out.len();
        out.push(node(m, depth, now, config));
        let below = walk(&m.id, depth + 1, by_parent, now, config, seen, out);
        let has_children = out.len() > at + 1;
        let c = &mut out[at];
        c.has_children = has_children;
        c.subtree.add(&below);
        let own = c.subtree;
        sum.add(&own);
    }
    sum
}

#[cfg(test)]
#[path = "children_tests.rs"]
mod tests;
