use super::*;
use tuse_config::{Connect, Function, Hook};

#[test]
fn renders_all_sections_without_secrets() {
    let mut cfg = Config {
        model: "p/m".into(),
        systems: vec!["Be helpful.".into()],
        models: vec![ModelDef {
            name: "p/m".into(),
            window: 100_000,
            ..Default::default()
        }],
        commands: vec!["hello".into()],
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
    });
    cfg.prompts.insert("rev".into(), "Review code".into());
    cfg.functions.insert(
        "helper".into(),
        Function {
            args: "x".into(),
            description: "Help".into(),
            show_agent: true,
        },
    );
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
            pnl.scroll = off;
            pnl.lines(80, 16, &p)
        })
        .flatten()
        .map(|s| s.text)
        .collect();
    for s in
        "Settings|Theme|Models|System prompts|Prompts|Functions|Hooks|Commands|Plugins".split('|')
    {
        assert!(text.contains(s), "missing {s}");
    }
    assert!(text.contains("/hello") && text.contains("#rev") && text.contains("helper(x)"));
    assert!(text.contains("me/kit") && text.contains("@v1 abc1234"));
    assert!(!text.contains("sk-secret"));
}
