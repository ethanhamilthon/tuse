mod common;
use tuse_core::{HOST_FUNCTIONS, Mode, Options, Session, Start};

#[test]
fn docs_cover_the_host_functions_and_the_vm_exports_them() {
    let functions = tuse_core::docs(Some("lua"));
    let config = tuse_core::docs(Some("config"));
    for name in HOST_FUNCTIONS {
        let documented = functions.contains(&format!("`{name}("))
            || functions.contains(&format!("`{name}."))
            || functions.contains(&format!("and `{name}("));
        assert!(documented, "docs(\"lua\") misses `{name}`");
        assert!(
            config.contains(&format!("`{name}`")) || name.starts_with("task_"),
            "docs(\"config\") require(\"tuse\") row misses `{name}`"
        );
    }
    let addr = common::provider(vec![common::Reply::Text("ok")]);
    let (dir, cfg) = common::setup("contract", &addr, "");
    let list = HOST_FUNCTIONS
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(",");
    std::fs::write(
        &cfg,
        format!(
            "local t = require('tuse')
for _, n in ipairs({{ {list} }}) do assert(t[n] ~= nil, n) end
local n = 0
for k in pairs(t) do n = n + 1 end
assert(n == {}, 'require(\"tuse\") exports ' .. n)
return {{}}",
            HOST_FUNCTIONS.len()
        ),
    )
    .unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
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
    keys.extend(json_keys(&tuse_config::UiSettings::default()));
    keys.extend(json_keys(&tuse_config::Theme::default()));
    keys.retain(|k| k != "lua");
    keys.extend(tuse_config::LuaTheme::TOKENS.iter().map(|t| t.to_string()));
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
}
