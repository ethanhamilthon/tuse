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
    pub cost: f64,
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
    let (v, unit) = match n {
        n if n >= 1_000_000 => (n as f64 / 1e6, "m"),
        n if n >= 1000 => (n as f64 / 1e3, "k"),
        n => return n.to_string(),
    };
    let s = format!("{v:.1}");
    format!("{}{unit}", s.strip_suffix(".0").unwrap_or(&s))
}

fn find_model<'a>(models: &'a [ModelDef], full: &str) -> Option<&'a ModelDef> {
    models.iter().rev().find(|m| m.name == full)
}

impl Stats {
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
