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
    let window = app
        .max_size
        .unwrap_or_else(|| config.model_def(&model).map_or(0, |d| d.window));
    if window > 0 || stats.context > 0 {
        parts.push(crate::maxsize::usage(stats.context, window));
    }
    if stats.priced {
        parts.push(format!("${:.2}", stats.cost));
    }
    parts.join(LABEL_SEP)
}
