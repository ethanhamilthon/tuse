mod common;
use tuse_config::{HOST_FUNCTIONS, HOST_GROUPS};
use tuse_core::{Mode, Options, Session, Start};

#[test]
fn docs_cover_the_host_functions_and_the_vm_exports_them() {
    let functions = tuse_core::docs(Some("lua"));
    let config = tuse_core::docs(Some("config"));
    for name in HOST_FUNCTIONS {
        let documented = functions.contains(&format!("`{name}("))
            || functions.contains(&format!("`{name}{{"))
            || functions.contains(&format!("`{name}."))
            || functions.contains(&format!("`{name}`"));
        assert!(documented, "docs(\"lua\") misses `{name}`");
        let top = name.split('.').next().unwrap();
        assert!(
            config.contains(&format!("`{top}`")),
            "docs(\"config\") require(\"tuse\") row misses `{top}` (from `{name}`)"
        );
    }
    let addr = common::provider(vec![common::Reply::Text("ok")]);
    let (dir, cfg) = common::setup("contract", &addr, "");
    let list = HOST_FUNCTIONS
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(",");
    let top_count = HOST_GROUPS.len() + 5;
    std::fs::write(
        &cfg,
        format!(
            "local t = require('tuse')
for _, path in ipairs({{ {list} }}) do
  local g, k = path:match('^([^.]+)%.(.+)$')
  if g then
    assert(type(t[g]) == 'table' and t[g][k] ~= nil, path)
  else
    assert(t[path] ~= nil, path)
  end
end
assert(t.inspect == nil and t.env == nil)
assert(#t.docs.topics == 8 and type(t.docs.lua) == 'string')
local n = 0
for _ in pairs(t) do n = n + 1 end
assert(n == {top_count}, 'require(\"tuse\") exports ' .. n)
return {{}}"
        ),
    )
    .unwrap();
    let s = Session::open({
        unsafe {
            std::env::set_var("TUSE_GLOBAL_CONFIG", cfg);
        }
        Options {
            cwd: dir.clone(),
            mode: Mode::Json,
            resume: None,
            start: Start::Launch,
            flags: Default::default(),
        }
    })
    .unwrap();
    s.close();
    common::cleanup(&dir);
}

fn json_keys<T: serde::Serialize>(value: &T) -> Vec<String> {
    serde_json::to_value(value)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

#[test]
fn docs_cover_the_config_keys_tokens_events_and_roles() {
    let config = tuse_core::docs(Some("config"));
    let lua = tuse_core::docs(Some("lua"));
    let session = tuse_core::docs(Some("session"));
    let mut keys = json_keys(&tuse_config::Config::default());
    keys.retain(|k| k != "path");
    keys.extend(json_keys(&tuse_config::Ui::default()));
    keys.extend(json_keys(&tuse_config::Lines::default()));
    keys.extend(json_keys(&tuse_config::Space::default()));
    keys.extend(json_keys(&tuse_config::Theme::default()));
    keys.extend(tuse_config::CodeTheme::TOKENS.iter().map(|t| t.to_string()));
    keys.extend(
        tuse_config::MarkdownTheme::TOKENS
            .iter()
            .map(|t| t.to_string()),
    );
    for key in keys {
        assert!(
            config.contains(&format!("`{key}`")),
            "docs(\"config\") misses `{key}`"
        );
    }
    for event in tuse_config::HOOK_EVENTS {
        assert!(
            lua.contains(&format!("`{event}`")),
            "docs(\"lua\") misses `{event}`"
        );
    }
    for role in tuse_config::HISTORY_ROLES {
        assert!(
            session.contains(&format!("`{role}`")),
            "docs(\"session\") misses `{role}`"
        );
    }
    for color in ["gray", "dark_gray"] {
        assert!(
            config.contains(&format!("`{color}`")),
            "docs(\"config\") misses `{color}`"
        );
    }
    for topic_name in [
        "usage", "connect", "config", "lua", "session", "json", "examples",
    ] {
        let page = tuse_core::docs(Some(topic_name));
        for retired in [
            "settings.open",
            "`surface`",
            "tuse --json",
            "list_max_height",
            "input_max_height",
            "pad_x",
            "session.snapshot",
            "session.info",
            "session.stats",
            "model.get",
            "session().name",
            "configpath",
            "usage.cached",
            "require(\"global\")",
            "require(\"plugins\")",
            "require(\"plugin:",
            "tuse connect",
            "tuse disconnect",
            "tuse plugins",
            "`/connect`",
            "`/disconnect`",
        ] {
            assert!(
                !page.contains(retired),
                "docs/{topic_name}.md still mentions `{retired}`"
            );
        }
    }
    let json_doc = tuse_core::docs(Some("json"));
    assert!(
        !json_doc.contains("- `session.resume`"),
        "docs/json.md must not list retired RPC method session.resume"
    );
    assert!(
        !tuse_core::docs(Some("connect")).contains("recomputes"),
        "docs/connect.md should not claim /reload recomputes cost"
    );
}
