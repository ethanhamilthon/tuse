use crate::app::App;
use crate::render::duration;
use crate::style::{LABEL_SEP, Palette, SPINNER, STATUS_MIN_MS, STEERING_LABEL, Style};
use crate::text::{Line, span};
use std::time::{Duration, Instant};

/// The action shown in the working indicator. A new action replaces the shown one, but
/// every shown action stays for at least `STATUS_MIN_MS`; the newest one wins.
pub struct Hold {
    shown: String,
    since: Instant,
    latest: String,
}

impl Default for Hold {
    fn default() -> Self {
        Self {
            shown: String::new(),
            since: Instant::now(),
            latest: String::new(),
        }
    }
}

impl Hold {
    pub fn set(&mut self, text: String) {
        self.latest = text;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Moves to the newest action when the shown one is old enough; true when it changed.
    pub fn tick(&mut self) -> bool {
        let old = self.since.elapsed() >= Duration::from_millis(STATUS_MIN_MS);
        if self.shown == self.latest || !(old || self.shown.is_empty()) {
            return false;
        }
        self.shown = self.latest.clone();
        self.since = Instant::now();
        true
    }
}

/// The working indicator: spinner and time in `accent`, the current action in `muted`;
/// empty when idle.
pub fn working(app: &App) -> Line {
    let p = &app.palette;
    let Some((elapsed, _)) = app.session.turn_info() else {
        return vec![];
    };
    if app.steering.is_some() {
        return vec![span(STEERING_LABEL, Style::fg(p.muted))];
    }
    let spin = SPINNER[app.tick % SPINNER.len()];
    let mut line = vec![span(
        format!("{spin} {}", duration(elapsed)),
        Style::fg(p.accent),
    )];
    if !app.status.shown.is_empty() {
        line.push(span(
            format!("{LABEL_SEP}{}", app.status.shown),
            Style::fg(p.muted),
        ));
    }
    line
}

/// The model indicator: model, effort, context use and cost.
pub fn model(app: &App) -> Line {
    let config = app.session.config();
    let (model, effort) = app.session.model();
    let stats = app.session.stats();
    let window = config.model_def(&model).map_or(0, |d| d.window);
    let context = (window > 0 || stats.context > 0).then_some((stats.context, window));
    let cost = stats.priced.then_some(stats.cost);
    model_line(
        &app.palette,
        &config.display_name(&model),
        &effort,
        context,
        cost,
    )
}

/// All `muted` but the used context (`x` of `x/y`, `accent`); `y` is `?` while unknown.
pub fn model_line(
    p: &Palette,
    name: &str,
    effort: &str,
    context: Option<(u64, u64)>,
    cost: Option<f64>,
) -> Line {
    let muted = Style::fg(p.muted);
    let mut text = format!("{name}{LABEL_SEP}{effort}");
    let mut line = vec![];
    if let Some((used, window)) = context {
        text.push_str(LABEL_SEP);
        line.push(span(std::mem::take(&mut text), muted));
        line.push(span(short(used), Style::fg(p.accent)));
        let max = if window > 0 {
            short(window)
        } else {
            "?".into()
        };
        text = format!("/{max}");
    }
    if let Some(cost) = cost {
        text.push_str(&format!("{LABEL_SEP}${cost:.2}"));
    }
    line.push(span(text, muted));
    line
}

/// `5200` -> `5.2k`, `1_000_000` -> `1m`: one compact form for both sides of `used/max`.
pub fn short(n: u64) -> String {
    let (v, unit) = match n {
        n if n >= 1_000_000 => (n as f64 / 1e6, "m"),
        n if n >= 1000 => (n as f64 / 1e3, "k"),
        n => return n.to_string(),
    };
    let s = format!("{v:.1}");
    format!("{}{unit}", s.strip_suffix(".0").unwrap_or(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_action_stays_for_the_minimum_time() {
        let mut h = Hold::default();
        h.set("reading a...".into());
        assert!(h.tick());
        assert_eq!(h.shown, "reading a...");
        h.set("writing b...".into());
        assert!(!h.tick(), "too early to replace");
        h.since -= Duration::from_millis(STATUS_MIN_MS);
        assert!(h.tick());
        assert_eq!(h.shown, "writing b...");
    }
    #[test]
    fn model_line_colors_only_the_used_context() {
        let p = Palette::from_config(&tuse_config::Config::default());
        let (m, a) = (Style::fg(p.muted), Style::fg(p.accent));
        assert_eq!(
            model_line(&p, "m", "high", Some((5200, 200_000)), Some(1.5)),
            [
                span("m · high · ", m),
                span("5.2k", a),
                span("/200k · $1.50", m)
            ]
        );
        assert_eq!(
            model_line(&p, "m", "low", Some((10, 0)), None),
            [span("m · low · ", m), span("10", a), span("/?", m)]
        );
        assert_eq!(model_line(&p, "m", "low", None, None), [span("m · low", m)]);
    }

    #[test]
    fn sizes_format() {
        assert_eq!(short(5200), "5.2k");
        assert_eq!(short(1_000_000), "1m");
        assert_eq!(short(200_000), "200k");
        assert_eq!(short(999), "999");
    }
}
