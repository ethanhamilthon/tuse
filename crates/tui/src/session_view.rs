use crate::panel::Panel;
use crate::style::{Palette, Style};
use crate::text::{Line, span};
use tuse_core::{Session, SessionSnapshot, tokens};

pub fn panel(session: &Session, p: &Palette) -> Panel {
    let snap = session.snapshot(false);
    let def = snap.model.as_deref().and_then(|m| session.model_def(m));
    let compact = session.config().compact_at(def.as_ref());
    panel_from_snapshot(&snap, compact, p)
}

pub(crate) fn panel_from_snapshot(s: &SessionSnapshot, compact: u64, p: &Palette) -> Panel {
    let key = Style::fg(p.border);
    let fg = Style::fg(p.fg);
    let dim = Style::fg(p.dim);
    let info = Style::fg(p.info);
    let accent = Style::fg(p.accent).bold();
    let ok = Style::fg(p.ok).bold();

    let row = |label: &str, mut rest: Line| -> Line {
        let mut out = vec![span(format!("{label:<13}"), key)];
        out.append(&mut rest);
        out
    };
    let opt = |v: Option<&str>, st: Style| match v {
        Some(x) => vec![span(x.to_string(), st)],
        None => vec![span("(none)", dim)],
    };
    let path_row = |label: &str, v: Option<&str>| match v {
        Some(path) => row(label, vec![crate::links::path_span(path, path, fg, p)]),
        None => row(label, vec![span("(none)", dim)]),
    };

    let ctx_used = s.ctx.unwrap_or(0);
    let mut ctx_spans = vec![span(tokens(ctx_used), accent)];
    if let Some(size) = s.ctx_size {
        ctx_spans.push(span(" / ", dim));
        ctx_spans.push(span(
            format!(
                "{} ({:.1}%)",
                tokens(size),
                ctx_used as f64 * 100.0 / size as f64
            ),
            info,
        ));
    } else {
        ctx_spans.push(span(" / ?", dim));
    }
    if compact > 0 {
        ctx_spans.push(span(format!(", compacts at {}", tokens(compact)), dim));
    }

    let model_info_spans = match &s.model_info {
        Some(mi) => {
            let mut parts = vec![];
            if let Some(d) = &mi.display {
                parts.push(d.clone());
            }
            if let Some(w) = mi.window {
                parts.push(format!("{} window", tokens(w)));
            }
            if let Some(c) = mi.compact_at {
                parts.push(if c == 0 {
                    "compact off".into()
                } else {
                    format!("compact {}", tokens(c))
                });
            }
            if let Some(pr) = &mi.prices {
                parts.push(format!(
                    "${}/${} (cache ${}/${}) per 1M",
                    pr.input, pr.output, pr.cache_read, pr.cache_write
                ));
            }
            vec![span(
                if parts.is_empty() {
                    mi.name.clone()
                } else {
                    parts.join(" · ")
                },
                fg,
            )]
        }
        None => vec![span("(none)", dim)],
    };

    let mut lines = vec![
        row("ID", opt(s.id.as_deref(), dim)),
        row("Name", opt(s.name.as_deref(), fg)),
        row("Mode", vec![span(s.mode, info)]),
        path_row("CWD", Some(&s.cwd)),
        path_row("Home", Some(&s.home)),
        path_row("Config", s.configpath.as_deref()),
        row("Provider", opt(s.provider.as_deref(), fg)),
        row("Model", opt(s.model.as_deref(), fg)),
        row("Effort", opt(s.effort.as_deref(), info)),
        row("Model info", model_info_spans),
        row("Context", ctx_spans),
        row(
            "Turns",
            vec![
                span(s.turns.unwrap_or(0).to_string(), fg),
                span(", steps ", dim),
                span(s.steps.unwrap_or(0).to_string(), fg),
            ],
        ),
    ];

    if let Some(u) = &s.usage {
        lines.push(row(
            "Usage",
            vec![
                span("in ", dim),
                span(tokens(u.input), fg),
                span(" (cache read ", dim),
                span(tokens(u.cache_read), fg),
                span(", cache write ", dim),
                span(tokens(u.cache_write), fg),
                span("), out ", dim),
                span(tokens(u.output), fg),
            ],
        ));
        lines.push(row(
            "Cost",
            match u.cost {
                Some(c) => {
                    let llm = u.llm_cost.unwrap_or(0.0);
                    vec![
                        span(format!("${c:.4}"), ok),
                        span(format!(" (${:.4} + ${llm:.4} llm)", c - llm), dim),
                    ]
                }
                None => vec![span("(unknown)", dim)],
            },
        ));
    }
    Panel::styled(lines).with_kind(crate::panel::PanelKind::Session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_panel_shows_every_snapshot_field_except_trace() {
        let app = crate::app::test_app("session-view");
        let p = &app.palette;
        let mut pnl = panel(&app.session, p);
        let lines = pnl.lines(100, 24, p);
        let text: String = lines.iter().flatten().map(|s| s.text.as_str()).collect();
        for label in [
            "ID",
            "Name",
            "Mode",
            "CWD",
            "Home",
            "Config",
            "Provider",
            "Model",
            "Effort",
            "Model info",
            "Context",
            "Turns",
            "Usage",
            "Cost",
        ] {
            assert!(text.contains(label), "missing {label} in {text}");
        }
        assert!(text.contains(&app.session.id));
        assert!(text.contains("tui"));
        crate::app::cleanup(app);
    }
}
