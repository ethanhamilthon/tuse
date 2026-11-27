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
    let l = &ui.lines;
    let sp = &ui.space;
    sec(
        "UI",
        vec![
            kv(
                "ui.lines",
                format!(
                    "tool={} output={} diff={} reasoning={} strings={} input={} list={}",
                    l.tool, l.output, l.diff, l.reasoning, l.strings, l.input, l.list
                ),
                dim,
            ),
            kv("ui.input", ui.input.as_str().into(), dim),
            kv(
                "ui.space",
                format!(
                    "padding={} steps={} messages={}",
                    sp.padding, sp.steps, sp.messages
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
    let max_w = |it: Box<dyn Iterator<Item = usize> + '_>| it.max().unwrap_or(0);
    let pad = |s: &str, w: usize| {
        format!(
            "{s}{}  ",
            " ".repeat(w.saturating_sub(crate::kit::text::width(s)))
        )
    };
    let pw = max_w(Box::new(
        cfg.connect.iter().map(|c| crate::kit::text::width(&c.name)),
    ));
    let tw = max_w(Box::new(
        cfg.connect
            .iter()
            .map(|c| crate::kit::text::width(c.api_type())),
    ));
    sec(
        "Providers",
        cfg.connect
            .iter()
            .map(|c| {
                vec![
                    span(format!("  {}", pad(&c.name, pw)), fg),
                    span(pad(c.api_type(), tw), info),
                    span(c.base_url.clone(), dim),
                ]
            })
            .collect(),
    );
    let plw = max_w(Box::new(
        cfg.plugins
            .iter()
            .map(|pl| crate::kit::text::width(&pl.name)),
    ));
    sec(
        "Plugins",
        cfg.plugins
            .iter()
            .map(|pl| {
                vec![
                    span(format!("  {}", pad(&pl.name, plw)), fg),
                    span(pl.version.clone(), dim),
                ]
            })
            .collect(),
    );
    let name_w = max_w(Box::new(
        cfg.models.iter().map(|m| crate::kit::text::width(&m.name)),
    ));
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
                if let Some(effs) = &m.efforts {
                    parts.push(if effs.is_empty() {
                        "efforts=none".into()
                    } else {
                        format!("efforts={}", effs.join(","))
                    });
                }
                if let Some(p) = &m.prices {
                    parts.push(format!(
                        "prices={}/{}/{}/{}",
                        p.input, p.output, p.cache_read, p.cache_write
                    ));
                }
                vec![
                    span(format!("  {}", pad(&m.name, name_w)), fg),
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
            .map(|(n, kind)| {
                let mut row = vec![span(format!("  {n}"), fg)];
                match kind {
                    tuse_config::FunctionKind::Custom => {}
                    tuse_config::FunctionKind::Override => row.push(span("  [override]", info)),
                    tuse_config::FunctionKind::Disabled => row.push(span("  [disabled]", dim)),
                }
                row
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
                    .into_iter()
                    .filter(|s| !s.is_empty())
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
            efforts: Some(vec![]),
            prices: Some(Default::default()),
        });
        cfg.plugins.push(tuse_config::PluginSummary {
            name: "demo".into(),
            version: "1.0.0".into(),
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
        for s in "UI|Providers|Plugins|Models|Theme|System prompts|Prompts|Functions|Hooks|Commands"
            .split('|')
        {
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
