use crate::{
    app::App,
    style::{RULE_CHAR, Style},
    text::{Line, span, wrap},
};

impl App {
    pub(crate) fn ask_active(&self) -> bool {
        self.ask_input.is_some() || self.picker.as_ref().is_some_and(|p| p.is_free())
    }
}

/// The question has its own viewport. Arrow keys still select options, and
/// Page Up/Down scroll the same question before and after choosing custom input.
pub(crate) fn lines(app: &mut App, w: usize, h: usize) -> Vec<Line> {
    let pk = app.picker.as_mut().filter(|pk| pk.is_free());
    let question = if let Some(pk) = &pk {
        &pk.title
    } else if let Some((_, q, _)) = &app.ask_input {
        q
    } else {
        return vec![];
    };
    let p = &app.palette;
    let style = Style::fg(p.accent).bold();
    let question: Vec<_> = crate::render::clean(question)
        .split('\n')
        .flat_map(|s| wrap(&vec![span(s, style)], w, &vec![], &vec![]))
        .collect();
    let controls = if pk.is_some() { 1 } else { 0 };
    let rule = usize::from(h > 1 + controls);
    let room = h.saturating_sub(rule);
    let mut options = vec![];
    if let Some(pk) = pk {
        let count = pk.shown.len().min(p.ui.list_max_height.max(1));
        let rows = count
            .min(room.saturating_sub(question.len()).max(1))
            .min(room);
        app.list_rows = rows.max(1);
        app.ask_controls = rows;
        options = crate::list::options(pk, rows, w, p);
    }
    let visible = room.saturating_sub(options.len());
    app.ask_scroll = app.ask_scroll.min(question.len().saturating_sub(visible));
    let mut lines = vec![];
    if rule > 0 {
        lines.push(vec![span(RULE_CHAR.repeat(w), Style::fg(p.border))]);
    }
    lines.extend(question.into_iter().skip(app.ask_scroll).take(visible));
    lines.extend(options);
    lines
}
