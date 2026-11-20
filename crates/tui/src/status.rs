use crate::app::App;
use crate::render::duration;
use crate::style::{LABEL_SEP, SPINNER, STEERING_LABEL};

/// The working indicator: spinner, time and tool title; empty when idle.
pub fn working(app: &App) -> String {
    let Some((elapsed, status)) = app.session.turn_info() else {
        return String::new();
    };
    if app.steering.is_some() {
        return STEERING_LABEL.to_string();
    }
    let spin = SPINNER[app.tick % SPINNER.len()];
    if status.is_empty() {
        return format!("{spin} {}", duration(elapsed));
    }
    format!("{spin} {}{LABEL_SEP}{status}", duration(elapsed))
}

/// The model indicator: model, effort, context use and cost.
pub fn model(app: &App) -> String {
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
    parts.join(LABEL_SEP)
}
