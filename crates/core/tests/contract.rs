//! The embedded `docs()` reference covers every Lua host function the VM exports.
mod common;
use tuse_core::{HOST_FUNCTIONS, Mode, Options, Session, Start};

#[test]
fn docs_cover_the_host_functions_and_the_vm_exports_them() {
    let functions = tuse_core::docs(Some("functions"));
    let config = tuse_core::docs(Some("config"));
    for name in HOST_FUNCTIONS {
        let documented = functions.contains(&format!("`{name}("))
            || functions.contains(&format!("`{name}.")) // json.encode
            || functions.contains(&format!("and `{name}(")); // inspect
        assert!(documented, "docs(\"functions\") misses `{name}`");
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
