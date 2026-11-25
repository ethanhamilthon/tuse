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
    let list = |v: &[String]| {
        if v.is_empty() {
            "(none)".to_string()
        } else {
            v.join(", ")
        }
    };
    sec(
        "Settings",
        vec![
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
            kv("ui.input_style", ui.input_style.as_str().into(), dim),
            kv("ui.hide_history", list(&ui.hide_history), dim),
            kv("ui.scope_models", list(&ui.scope_models), dim),
            kv(
                "open",
                list(
                    &cfg.settings
                        .open
                        .iter()
                        .map(|e| format!(".{e}"))
                        .collect::<Vec<_>>(),
                ),
                info,
            ),
        ],
    );
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
                    span(format!("  {}  ", m.name), fg),
                    span(parts.join(" "), dim),
                ]
            })
            .collect(),
    );

    let t = &cfg.theme;
    let mut theme_pairs = vec![
        ("bg".to_string(), t.bg.as_str()),
        ("surface".to_string(), t.surface.as_str()),
        ("fg".to_string(), t.fg.as_str()),
        ("user".to_string(), t.user.as_str()),
        ("dim".to_string(), t.dim.as_str()),
        ("muted".to_string(), t.muted.as_str()),
        ("border".to_string(), t.border.as_str()),
        ("accent".to_string(), t.accent.as_str()),
        ("ok".to_string(), t.ok.as_str()),
        ("err".to_string(), t.err.as_str()),
        ("info".to_string(), t.info.as_str()),
    ];
    let lua_values = t.lua.values();
    for (name, value) in tuse_config::LuaTheme::TOKENS.iter().zip(lua_values) {
        theme_pairs.push((format!("lua.{name}"), value));
    }
    sec(
        "Theme",
        theme_pairs
            .into_iter()
            .map(|(n, v)| {
                let linked = t.named(v).unwrap_or(v);
                let sw = color(if linked.trim() == "accent" {
                    &t.accent
                } else {
                    linked
                });
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
            .map(|n| vec![span(format!("  /{n}"), info)])
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
            path: "/home/me/.config/tuse/init.lua".into(),
            ..Default::default()
        };
        cfg.settings.open = vec!["md".into()];
        cfg.settings.ui.hide_history = vec!["usage".into()];
        cfg.settings.ui.scope_models = vec!["p/*".into()];
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
        for s in "Settings|Models|Theme|System prompts|Prompts|Functions|Hooks|Commands".split('|')
        {
            assert!(text.contains(s), "missing {s}");
        }
        let json: serde_json::Value = serde_json::from_str(&cfg.to_json()).unwrap();
        let mut keys = vec![];
        for (k, v) in json.as_object().unwrap() {
            keys.push(k.clone());
            if k == "settings" {
                for (k2, v2) in v.as_object().unwrap() {
                    keys.push(k2.clone());
                    if k2 == "ui" {
                        keys.extend(v2.as_object().unwrap().keys().cloned());
                    }
                }
            }
        }
        let flat = text.replace(' ', "");
        for k in keys {
            let shown = match k.as_str() {
                "ui" | "settings" | "path" | "theme" => continue,
                "tool_line" | "diff_line" | "output_line" | "reasoning_line" => {
                    k.trim_end_matches("_line").to_string()
                }
                "scroll_step" => "scroll".into(),
                "message_gap" => "gap".into(),
                "list_max_height" => "list_max".into(),
                "input_max_height" => "input_max".into(),
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
        assert!(text.contains("p/big") && text.contains("fn(apply)") && text.contains("p/*"));
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
