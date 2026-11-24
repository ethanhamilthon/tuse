use crate::kit::style::{Palette, Style, color};
use crate::kit::text::{Line, span};
use crate::ui::panel::Panel;
use tuse_config::Config;
use tuse_core::tokens;

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
    let file_row = if cfg.path.is_empty() {
        vec![span("  (none, defaults)", dim)]
    } else {
        vec![
            span("  ", fg),
            crate::kit::links::path_span(&cfg.path, &cfg.path, fg, p),
        ]
    };
    sec("Config file", vec![file_row]);
    sec(
        "Settings",
        vec![
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
        ],
    );

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
            .map(|(n, kind)| match kind {
                tuse_config::FunctionKind::Custom => vec![span(format!("  {n}"), fg)],
                tuse_config::FunctionKind::Override => {
                    vec![span(format!("  {n}"), fg), span("  [override]", info)]
                }
                tuse_config::FunctionKind::Disabled => {
                    vec![span(format!("  {n}"), fg), span("  [disabled]", dim)]
                }
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
        "Open",
        cfg.open
            .iter()
            .map(|ext| vec![span(format!("  .{ext}"), info)])
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
                    crate::kit::links::path_span(&pl.dir, &pl.dir, dim, p),
                ]
            })
            .collect(),
    );
    Panel::styled(lines).with_kind(crate::ui::panel::PanelKind::Config)
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuse_config::{Connect, FunctionKind, Hook};

    #[test]
    fn renders_all_sections_without_secrets() {
        let mut cfg = Config {
            systems: vec!["Be helpful.".into()],
            commands: vec!["hello".into()],
            open: vec!["md".into()],
            path: "/home/me/.config/tuse/init.lua".into(),
            plugins: vec![tuse_config::PluginInfo {
                name: "me/kit".into(),
                dir: "/p/kit".into(),
                git_ref: Some("v1".into()),
                commit: Some("abc1234".into()),
            }],
            ..Default::default()
        };
        cfg.connect.push(Connect {
            name: "p".into(),
            base_url: "https://x".into(),
            api_key: "sk-secret".into(),
            ..Default::default()
        });
        cfg.prompts.insert("rev".into(), "Review code".into());
        cfg.functions.insert("helper".into(), FunctionKind::Custom);
        cfg.functions.insert("sh".into(), FunctionKind::Override);
        cfg.functions
            .insert("delete".into(), FunctionKind::Disabled);
        cfg.hooks.insert(
            "h".into(),
            Hook {
                events: vec!["user_message".into()],
            },
        );
        let p = Palette::from_config(&cfg);
        let mut pnl = panel(&cfg, &p);
        let text: String = (0..80)
            .step_by(10)
            .flat_map(|off| {
                pnl.scroll_by(off as isize - pnl.scroll() as isize);
                pnl.lines(80, 16, &p)
            })
            .flatten()
            .map(|s| s.text)
            .collect();
        for s in
            "Settings|Theme|System prompts|Prompts|Functions|Hooks|Commands|Open|Plugins".split('|')
        {
            assert!(text.contains(s), "missing {s}");
        }
        assert!(
            text.contains("/hello")
                && text.contains("#rev")
                && text.contains("helper")
                && text.contains("sh  [override]")
                && text.contains("delete  [disabled]")
        );
        assert!(text.contains(".md"));
        assert!(text.contains("Config file") && text.contains("/home/me/.config/tuse/init.lua"));
        assert!(text.contains("me/kit") && text.contains("@v1 abc1234"));
        assert!(!text.contains("sk-secret"));
    }
}
