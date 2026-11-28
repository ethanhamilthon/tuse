use tuse_api::Usage;
use tuse_config::Prices;

pub fn cost(u: &Usage, p: &Prices) -> f64 {
    let plain = u.input.saturating_sub(u.cache_read + u.cache_write);
    (plain as f64 * p.input
        + u.cache_read as f64 * p.cache_read
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

impl Stats {
    pub(crate) fn of(m: &crate::storage::store::Meta, records: &[crate::Record]) -> Self {
        let mut s = Self::default();
        s.update_meta(m);
        s.add_records(records);
        s
    }

    pub(crate) fn update_meta(&mut self, m: &crate::storage::store::Meta) {
        self.totals = m.usage;
        self.cost = m.cost;
        self.llm_cost = m.llm_cost;
        self.priced = m.priced;
        self.context = m.ctx;
    }

    pub(crate) fn add_records(&mut self, records: &[crate::Record]) {
        use crate::Record;
        for r in records {
            match r {
                Record::Usage { llm: true, .. } => self.llm_requests += 1,
                Record::Usage { .. } => self.requests += 1,
                Record::Agent { .. } => self.steps += 1,
                Record::TurnEnd { calls, .. } => {
                    self.turns += 1;
                    self.calls += calls;
                }
                _ => {}
            }
        }
    }
}
