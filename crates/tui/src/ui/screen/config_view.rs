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
        Style::fg(p.muted),
        Style::fg(p.heading),
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

    let ui = &cfg.ui;
    let file_row = if cfg.path.is_empty() {
        vec![span("  (none, defaults)", dim)]
    } else {
        vec![
            span("  ", fg),
            crate::kit::links::path_span(&cfg.path, &cfg.path, fg, p),
        ]
    };
    sec("Config file", vec![file_row]);
    let list = |v: &[String]| {
        if v.is_empty() {
            "(none)".to_string()
        } else {
            v.join(", ")
        }
    };
    sec(
        "UI",
        vec![
            kv(
                "ui.lines",
                format!(
                    "tool={} output={} diff={} reasoning={} strings={} input={} list={}",
                    ui.lines.tool,
                    ui.lines.output,
                    ui.lines.diff,
                    ui.lines.reasoning,
                    ui.lines.strings,
                    ui.lines.input,
                    ui.lines.list
                ),
                dim,
            ),
            kv("ui.input", ui.input.as_str().into(), dim),
            kv(
                "ui.space",
                format!(
                    "padding={} steps={} messages={}",
                    ui.space.padding, ui.space.steps, ui.space.messages
                ),
                dim,
            ),
            kv("history.hide", list(&cfg.history.hide), dim),
            kv(
                "open",
                list(
                    &cfg.open
                        .iter()
                        .map(|e| tuse_config::open_label(e))
                        .collect::<Vec<_>>(),
                ),
                info,
            ),
        ],
    );
    let name_w = cfg
        .models
        .iter()
        .map(|m| crate::kit::text::width(&m.name))
        .max()
        .unwrap_or(0);
    sec(
        "Models",
        cfg.models
            .iter()
            .map(|m| {
                let mut parts = vec![];
                if !m.display.is_empty() {
                    parts.push(format!("display={}", m.display));
                }
                if m.window > 0 {
                    parts.push(format!("window={}", tokens(m.window)));
                }
                if let Some(p) = &m.prices {
                    parts.push(format!(
                        "prices={}/{}/{}/{}",
                        p.input, p.output, p.cache_read, p.cache_write
                    ));
                }
                vec![
                    span(
                        format!(
                            "  {}{}  ",
                            m.name,
                            " ".repeat(name_w - crate::kit::text::width(&m.name))
                        ),
                        fg,
                    ),
                    span(parts.join(" "), dim),
                ]
            })
            .collect(),
    );

    let t = &ui.theme;
    sec(
        "Theme",
        t.entries()
            .into_iter()
            .map(|(n, v)| {
                let sw = color(t.resolve(v));
                vec![
                    span(format!("  {n:<18}"), key),
                    span("■ ", Style::fg(sw)),
                    span(v, fg),
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
            .map(|(n, b)| match b.kind {
                tuse_config::PromptKind::Text => kv(&format!("#{n}"), first_line(&b.body), fg),
                tuse_config::PromptKind::Function => kv(
                    &format!("#{n}"),
                    format!("fn({})", b.args.join(" | ")),
                    info,
                ),
            })
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
            .map(|(n, c)| {
                let mut row = vec![span(format!("  /{n}"), info)];
                let args = c.args.join(" | ");
                let detail = [c.desc.as_str(), args.as_str()]
                    .iter()
                    .filter(|s| !s.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join("  ");
                if !detail.is_empty() {
                    row.push(span(format!("  {detail}"), dim));
                }
                row
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
    fn model_details_start_in_one_column() {
        let mut cfg = Config::default();
        for name in ["p/a", "p/much-longer"] {
            cfg.models.push(tuse_config::ModelDef {
                name: name.into(),
                window: 1_000_000,
                ..Default::default()
            });
        }
        let p = Palette::from_config(&cfg);
        let mut pnl = panel(&cfg, &p);
        let rows: Vec<String> = (0..60)
            .step_by(10)
            .flat_map(|off| {
                pnl.scroll_by(off as isize - pnl.scroll() as isize);
                pnl.lines(80, 16, &p)
            })
            .map(|l| l.into_iter().map(|s| s.text).collect::<String>())
            .filter(|r| r.contains("window="))
            .collect();
        assert!(rows.len() >= 2, "{rows:?}");
        assert_eq!(rows[0].find("window="), rows[1].find("window="));
    }

    #[test]
    fn renders_all_sections_without_secrets() {
        let mut cfg = Config {
            systems: vec!["Be helpful.".into()],
            commands: [("hello".to_string(), Default::default())].into(),
            path: "/home/me/.config/tuse/init.lua".into(),
            ..Default::default()
        };
        cfg.open = vec!["md".into()];
        cfg.history.hide = vec!["usage".into()];
        cfg.models.push(tuse_config::ModelDef {
            name: "p/big".into(),
            display: "Big".into(),
            window: 200_000,
            prices: Some(Default::default()),
        });
        cfg.prompts.insert(
            "spec".into(),
            tuse_config::Prompt {
                kind: tuse_config::PromptKind::Function,
                body: String::new(),
                args: vec!["apply".into()],
            },
        );
        cfg.connect.push(Connect {
            name: "p".into(),
            base_url: "https://x".into(),
            api_key: "sk-secret".into(),
            ..Default::default()
        });
        cfg.prompts.insert(
            "rev".into(),
            tuse_config::Prompt {
                body: "Review code".into(),
                ..Default::default()
            },
        );
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
        let text: String = (0..200)
            .step_by(10)
            .flat_map(|off| {
                pnl.scroll_by(off as isize - pnl.scroll() as isize);
                pnl.lines(80, 16, &p)
            })
            .flatten()
            .map(|s| s.text)
            .collect();
        for s in "UI|Models|Theme|System prompts|Prompts|Functions|Hooks|Commands".split('|') {
            assert!(text.contains(s), "missing {s}");
        }
        let json: serde_json::Value = serde_json::from_str(&cfg.to_json()).unwrap();
        let mut keys = vec![];
        for (k, v) in json.as_object().unwrap() {
            keys.push(k.clone());
            if k == "ui" {
                for (k2, v2) in v.as_object().unwrap() {
                    keys.push(k2.clone());
                    if k2 == "lines" || k2 == "space" {
                        keys.extend(v2.as_object().unwrap().keys().cloned());
                    }
                }
            }
            if k == "history" {
                keys.extend(v.as_object().unwrap().keys().cloned());
            }
        }
        let flat = text.replace(' ', "");
        for k in keys {
            let shown = match k.as_str() {
                "ui" | "history" | "path" | "theme" | "lines" | "space" => continue,
                "systems" => "Systemprompts".into(),
                k => k.replace('_', ""),
            };
            assert!(
                flat.to_lowercase()
                    .replace('_', "")
                    .contains(&shown.to_lowercase().replace('_', "")),
                "/config misses `{k}`"
            );
        }
        assert!(text.contains("p/big") && text.contains("fn(apply)"));
        assert!(
            text.contains("/hello")
                && text.contains("#rev")
                && text.contains("helper")
                && text.contains("sh  [override]")
                && text.contains("delete  [disabled]")
        );
        assert!(text.contains(".md"));
        assert!(text.contains("Config file") && text.contains("/home/me/.config/tuse/init.lua"));
        assert!(!text.contains("sk-secret"));
    }
}
