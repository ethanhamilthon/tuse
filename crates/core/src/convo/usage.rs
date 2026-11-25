use tuse_api::Usage;
use tuse_config::Prices;

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

impl Stats {
    pub(crate) fn of(m: &crate::storage::store::Meta, records: &[crate::Record]) -> Self {
        use crate::convo::record::Record;
        let mut s = Self {
            totals: m.usage,
            cost: m.cost,
            llm_cost: m.llm_cost,
            priced: m.priced,
            context: m.ctx,
            ..Self::default()
        };
        for r in records {
            match r {
                Record::Usage { llm: true, .. } => s.llm_requests += 1,
                Record::Usage { .. } => s.requests += 1,
                Record::Assistant { .. } => s.steps += 1,
                Record::TurnEnd { calls, .. } => {
                    s.turns += 1;
                    s.calls += calls;
                }
                _ => {}
            }
        }
        s
    }
}
