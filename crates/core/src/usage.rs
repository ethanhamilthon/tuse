use tuse_api::Usage;
use tuse_config::{ModelDef, Prices};

pub fn cost(u: &Usage, p: &Prices) -> f64 {
    let plain = u.input.saturating_sub(u.cached + u.cache_write);
    (plain as f64 * p.input
        + u.cached as f64 * p.cache_read
        + u.cache_write as f64 * p.cache_write
        + u.output as f64 * p.output)
        / 1_000_000.0
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub totals: Usage,
    /// `$main + $llm`: the whole session.
    pub cost: f64,
    /// What `llm()` requests cost (their tokens are in `totals` too).
    pub llm_cost: f64,
    pub llm_requests: usize,
    pub priced: bool,
    pub requests: usize,
    pub steps: usize,
    pub context: u64,
    pub turns: usize,
    pub calls: usize,
}

pub fn tokens(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.2}M", n as f64 / 1e6),
        n if n >= 1000 => format!("{:.1}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

fn find_model<'a>(models: &'a [ModelDef], full: &str) -> Option<&'a ModelDef> {
    models.iter().rev().find(|m| m.name == full)
}

impl Stats {
    /// Adds one record; `Usage`, `Assistant` (steps), and `TurnEnd` count.
    pub(crate) fn add(&mut self, r: &crate::record::Record, models: &[ModelDef]) {
        use crate::record::Record;
        match r {
            Record::Usage {
                model,
                usage: u,
                llm,
            } => {
                self.totals.add(u);
                if *llm {
                    self.llm_requests += 1;
                } else {
                    self.requests += 1;
                }
                if let Some(p) = find_model(models, model).and_then(|d| d.prices.as_ref()) {
                    let c = cost(u, p);
                    self.cost += c;
                    if *llm {
                        self.llm_cost += c;
                    }
                    self.priced = true;
                }
            }
            Record::Assistant { .. } => {
                self.steps += 1;
            }
            Record::TurnEnd { calls, .. } => {
                self.turns += 1;
                self.calls += calls;
            }
            _ => {}
        }
    }

    pub(crate) fn of(records: &[crate::record::Record], models: &[ModelDef]) -> Stats {
        let mut s = Stats::default();
        for r in records {
            s.add(r, models);
        }
        s
    }
}
