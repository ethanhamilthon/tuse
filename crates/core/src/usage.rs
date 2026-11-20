use tuse_api::Usage;
use tuse_config::Prices;

pub fn cost(u: &Usage, p: &Prices) -> f64 {
    let plain = u.input.saturating_sub(u.cached + u.cache_write);
    (plain as f64 * p.input
        + u.cached as f64 * p.cache
        + u.cache_write as f64 * p.cache_write
        + u.output as f64 * p.out)
        / 1_000_000.0
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub totals: Usage,
    pub cost: f64,
    pub priced: bool,
    pub requests: usize,
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

impl Stats {
    /// Adds one record; only `Usage` and `TurnEnd` count.
    pub(crate) fn add(&mut self, r: &crate::record::Record, config: &tuse_config::Config) {
        use crate::record::Record;
        match r {
            Record::Usage { model, usage: u } => {
                self.totals.add(u);
                self.requests += 1;
                if let Some(p) = config.model_def(model).and_then(|d| d.prices.as_ref()) {
                    self.cost += cost(u, p);
                    self.priced = true;
                }
            }
            Record::TurnEnd { calls, .. } => {
                self.turns += 1;
                self.calls += calls;
            }
            _ => {}
        }
    }

    pub(crate) fn of(records: &[crate::record::Record], config: &tuse_config::Config) -> Stats {
        let mut s = Stats::default();
        for r in records {
            s.add(r, config);
        }
        s
    }
}
