use crate::panel::Panel;
use crate::style::{Palette, Style, color};
use crate::text::{Line, span};
use tuse_config::{Config, ModelDef};
use tuse_core::tokens;

/// Styled `/config` panel using only the 10-token `Palette` (no secrets or Lua bodies).
pub fn panel(cfg: &Config, p: &Palette) -> Panel {
    let (head, key, fg, dim, info) = (
        Style::fg(p.accent).bold(),
        Style::fg(p.border),
        Style::fg(p.fg),
        Style::fg(p.dim),
        Style::fg(p.info),
    );
    let mut lines: Vec<Line> = vec![];
    let mut sec = |title: &str, rows: Vec<Line>| {
        if !lines.is_empty() {
            lines.push(vec![]);
        }
        lines.push(vec![span(title, head)]);
        if rows.is_empty() {
            lines.push(vec![span("  (none)", dim)]);
        } else {
            lines.extend(rows);
        }
    };
    let kv = |k: &str, v: String, st: Style| vec![span(format!("  {k:<18}"), key), span(v, st)];

    let compact = match cfg.settings.compact_at {
        Some(0) => "off".into(),
        Some(n) => tokens(n),
        None => "auto (60% window)".into(),
    };
    let ui = &cfg.settings.ui;
    let mut settings = vec![kv("model", cfg.default_model(), fg)];
    if !cfg.provider.is_empty() {
        settings.push(kv("provider", cfg.provider.clone(), fg));
    }
    settings.extend([
        kv("effort", cfg.default_effort(), info),
        kv("compact_at", compact, fg),
        kv(
            "max_image_bytes",
            cfg.settings.max_image_bytes.to_string(),
            fg,
        ),
        kv("retries", cfg.settings.retries.to_string(), fg),
        kv(
            "ui.lines",
            format!(
                "tool={} diff={} output={} reasoning={}",
                ui.tool_line, ui.diff_line, ui.output_line, ui.reasoning_line
            ),
            dim,
        ),
        kv(
            "ui.layout",
            format!(
                "fold_strings={} scroll={} pad_x={} gap={} list_max={} input_max={}",
                ui.fold_strings,
                ui.scroll_step,
                ui.pad_x,
                ui.message_gap,
                ui.list_max_height,
                ui.input_max_height
            ),
            dim,
        ),
    ]);
    sec("Settings", settings);

    let t = &cfg.theme;
    let theme_pairs = [
        ("bg", &t.bg),
        ("surface", &t.surface),
        ("fg", &t.fg),
        ("dim", &t.dim),
        ("muted", &t.muted),
        ("border", &t.border),
        ("accent", &t.accent),
        ("ok", &t.ok),
        ("err", &t.err),
        ("info", &t.info),
    ];
    sec(
        "Theme",
        theme_pairs
            .into_iter()
            .map(|(n, v)| {
                let sw = color(if v.trim() == "accent" { &t.accent } else { v });
                vec![
                    span(format!("  {n:<18}"), key),
                    span("■ ", Style::fg(sw)),
                    span(v.clone(), fg),
                ]
            })
            .collect(),
    );
    sec(
        "Models",
        cfg.models
            .iter()
            .map(|m| kv(&m.name, model_detail(m), fg))
            .collect(),
    );
    sec(
        "System prompts",
        cfg.systems
            .iter()
            .enumerate()
            .map(|(i, s)| kv(&format!("#{}", i + 1), first_line(s), fg))
            .collect(),
    );
    sec(
        "Prompts",
        cfg.prompts
            .iter()
            .map(|(n, b)| kv(&format!("#{n}"), first_line(b), fg))
            .collect(),
    );
    sec(
        "Functions",
        cfg.functions
            .iter()
            .map(|(n, f)| {
                let suf = if f.show_agent { "" } else { " [hidden]" };
                kv(
                    &format!("{n}({})", f.args),
                    format!("{}{suf}", f.description),
                    fg,
                )
            })
            .collect(),
    );
    sec(
        "Hooks",
        cfg.hooks
            .iter()
            .map(|(n, h)| kv(n, h.events.join(", "), info))
            .collect(),
    );
    sec(
        "Commands",
        cfg.commands
            .iter()
            .map(|n| vec![span(format!("  /{n}"), info)])
            .collect(),
    );
    sec(
        "Plugins",
        cfg.plugins
            .iter()
            .map(|pl| {
                let r = pl
                    .git_ref
                    .as_deref()
                    .map(|r| format!("@{r} "))
                    .unwrap_or_default();
                let at = pl.commit.as_deref().unwrap_or("local");
                vec![
                    span(format!("  {}", pl.name), info),
                    span(format!("  {r}{at}  "), fg),
                    span(pl.dir.clone(), dim),
                ]
            })
            .collect(),
    );
    Panel::styled(lines)
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn model_detail(m: &ModelDef) -> String {
    let mut parts = vec![];
    if !m.display.is_empty() {
        parts.push(m.display.clone());
    }
    if m.window > 0 {
        parts.push(format!("{} ctx", tokens(m.window)));
    }
    if let Some(c) = m.compact_at {
        parts.push(if c == 0 {
            "compact off".into()
        } else {
            format!("compact {}", tokens(c))
        });
    }
    if let Some(pr) = &m.prices {
        parts.push(format!(
            "${}/${} (cache ${}/${})",
            pr.input, pr.out, pr.cache, pr.cache_write
        ));
    }
    if parts.is_empty() {
        "-".into()
    } else {
        parts.join(" · ")
    }
}

#[cfg(test)]
#[path = "config_view_tests.rs"]
mod tests;
