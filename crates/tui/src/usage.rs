use crate::panel::Panel;
use crate::style::{Palette, Style};
use crate::text::{Line, span};
use tuse_core::{Session, tokens};

/// Styled `/usage` panel using the 10-color `Palette`.
pub fn panel(session: &Session, p: &Palette) -> Panel {
    let config = session.config();
    let (model, effort) = session.model();
    let st = session.stats();
    let def = config.model_def(&model);
    let window = def.map(|d| d.window).unwrap_or(0);
    let compact = config.compact_at(&model);
    let display = config.display_name(&model);

    let key = Style::fg(p.border);
    let fg = Style::fg(p.fg);
    let dim = Style::fg(p.dim);
    let info = Style::fg(p.info);
    let accent = Style::fg(p.accent).bold();
    let ok = Style::fg(p.ok).bold();

    let row = |label: &str, mut rest: Line| -> Line {
        let mut out = vec![span(format!("{label:<11}"), key)];
        out.append(&mut rest);
        out
    };

    let mut model_spans = vec![span(display.clone(), fg)];
    if display != model {
        model_spans.push(span(format!(" ({model})"), dim));
    }
    model_spans.push(span(", effort ", dim));
    model_spans.push(span(effort, info));

    let mut ctx_spans = vec![span(tokens(st.context), accent)];
    if window > 0 {
        ctx_spans.push(span(" / ", dim));
        ctx_spans.push(span(
            format!(
                "{} ({:.1}%)",
                tokens(window),
                st.context as f64 * 100.0 / window as f64
            ),
            info,
        ));
    }
    if compact > 0 {
        ctx_spans.push(span(format!(", compacts at {}", tokens(compact)), dim));
    }

    let t = &st.totals;
    let mut lines = vec![
        row("Session", vec![span(session.id.clone(), dim)]),
        row("Model", model_spans),
        row("Context", ctx_spans),
        row(
            "Turns",
            vec![
                span(st.turns.to_string(), fg),
                span(", tool calls ", dim),
                span(st.calls.to_string(), fg),
                span(", requests ", dim),
                span(st.requests.to_string(), fg),
            ],
        ),
        row(
            "Tokens",
            vec![
                span("in ", dim),
                span(tokens(t.input), fg),
                span(" (cached ", dim),
                span(tokens(t.cached), fg),
                span(", cache write ", dim),
                span(tokens(t.cache_write), fg),
                span("), out ", dim),
                span(tokens(t.output), fg),
            ],
        ),
    ];
    if st.priced {
        lines.push(row("Cost", vec![span(format!("${:.4}", st.cost), ok)]));
    }
    Panel::styled("Usage", lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_panel_styles_keys_and_context() {
        let app = crate::app::test_app("usage", true);
        let p = &app.palette;
        let mut pnl = panel(&app.session, p);
        let lines = pnl.lines(80, 16, p);
        // Rule + Title + 5 rows (Session, Model, Context, Turns, Tokens)
        assert_eq!(lines.len(), 7);
        // Key span on Session row uses border color; Context value uses accent bold.
        assert_eq!(lines[2][0].style.fg, p.border);
        assert_eq!(lines[4][0].style.fg, p.border);
        assert_eq!(lines[4][1].style.fg, p.accent);
        assert!(lines[4][1].style.bold);
        crate::app::cleanup(app);
    }
}
