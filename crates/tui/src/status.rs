use crate::app::App;
use crate::render::duration;

/// The working indicator: spinner, time and tool calls; empty when the agent is idle.
pub fn working(app: &App) -> String {
    let t = &app.palette.t;
    let Some((elapsed, calls)) = app.session.turn_info() else {
        return String::new();
    };
    if app.steering.is_some() {
        return t.steering_label.clone();
    }
    let frames = &t.spinner;
    let spin = frames
        .get(app.tick % frames.len().max(1))
        .cloned()
        .unwrap_or_default();
    format!("{spin} {}{}{calls} calls", duration(elapsed), t.label_sep)
}

/// The model indicator: model, effort, context use and cost.
pub fn model(app: &App) -> String {
    let t = &app.palette.t;
    let config = app.session.config();
    let (model, effort) = app.session.model();
    let mut parts = vec![config.display_name(&model), effort];
    let stats = app.session.stats();
    let window = config.model_def(&model).map(|d| d.window).unwrap_or(0);
    if window > 0 {
        parts.push(format!(
            "{:.0}%",
            stats.context as f64 * 100.0 / window as f64
        ));
    } else if stats.context > 0 {
        parts.push(format!("{} tok", tuse_core::tokens(stats.context)));
    }
    if stats.priced {
        parts.push(format!("${:.2}", stats.cost));
    }
    parts.join(&t.label_sep)
}
