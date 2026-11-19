use tuse_api::Usage;
use tuse_config::{Config, Prices};

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

pub fn report(id: &str, model: &str, effort: &str, config: &Config, st: &Stats) -> String {
    let def = config.model_def(model);
    let window = def.map(|d| d.window).unwrap_or(0);
    let compact = config.compact_at(model);
    let mut ctx = format!("Context    {}", tokens(st.context));
    if window > 0 {
        ctx.push_str(&format!(
            " / {} ({:.1}%)",
            tokens(window),
            st.context as f64 * 100.0 / window as f64
        ));
    }
    if compact > 0 {
        ctx.push_str(&format!(", compacts at {}", tokens(compact)));
    }
    let t = &st.totals;
    let mut lines = vec![
        format!("Session    {id}"),
        format!(
            "Model      {} ({model}), effort {effort}",
            config.display_name(model)
        ),
        ctx,
        format!(
            "Turns      {}, tool calls {}, requests {}",
            st.turns, st.calls, st.requests
        ),
        format!(
            "Tokens     in {} (cached {}, cache write {}), out {}",
            tokens(t.input),
            tokens(t.cached),
            tokens(t.cache_write),
            tokens(t.output)
        ),
    ];
    if st.priced {
        lines.push(format!("Cost       ${:.4}", st.cost));
    }
    lines.join("\n")
}
