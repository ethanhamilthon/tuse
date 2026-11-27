mod common;

use std::fs;
use tuse_core::{Mode, Options, Session, Start};

const BASE: &str = "https://github.com/ethanhamilthon/tuse/blob/main/examples/";

#[test]
fn examples_load_and_are_linked_from_the_docs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let page = fs::read_to_string(root.join("docs/examples.md")).unwrap();
    let mut files: Vec<String> = fs::read_dir(root.join("examples"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".lua"))
        .collect();
    files.sort();
    for f in &files {
        assert!(
            page.contains(&format!("{BASE}{f}")),
            "docs/examples.md misses {f}"
        );
    }
    for link in page.split(BASE).skip(1) {
        let name: String = link
            .chars()
            .take_while(|c| *c != ')' && !c.is_whitespace())
            .collect();
        assert!(
            files.contains(&name),
            "docs/examples.md links missing {name}"
        );
    }

    let addr = common::provider(vec![]);
    let (dir, cfg) = common::setup("examples", &addr, "");
    let ex = root.join("examples").canonicalize().unwrap();
    let entries: Vec<String> = files
        .iter()
        .map(|f| f.trim_end_matches(".lua").to_string())
        .filter(|m| !["themes", "project", "mcp"].contains(&m.as_str()))
        .map(|m| {
            let arg = match m.as_str() {
                "rename" | "websearch" => "{ model = 'x' }",
                _ => "nil",
            };
            format!("  ex_{m} = load_ex('{}/{m}', {arg}),", ex.display())
        })
        .collect();
    fs::write(
        &cfg,
        format!(
            "local function load_ex(p, a)\n  local m = require(p)\n  if type(m) == 'function' then return m(a) end\n  return m\nend\n\
             return {{\n{}\n  ex_theme = {{ ui = {{ theme = require('{e}/themes').gruvbox }} }},\n\
             ex_mcp = load_ex('{e}/mcp', {{ cmd = {{ 'true' }} }}),\n}}\n",
            entries.join("\n"),
            e = ex.display()
        ),
    )
    .unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let c = s.config();
    assert_eq!(c.prompts["openspec"].args[1], "propose");
    assert!(c.open.contains(&"md".to_string()));
    let got = s
        .resolve_prompts(&["openspec propose".into()], true)
        .unwrap();
    assert!(got[0].2.starts_with("OpenSpec propose:"), "{got:?}");
    assert!(s.resolve_prompts(&["openspec nope".into()], true).is_err());
    s.close();

    fs::write(
        &cfg,
        format!("return dofile('{}/project.lua')", ex.display()),
    )
    .unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    assert_eq!(s.config().history.hide, ["usage", "flash"]);
    s.close();

    let s = Session::open(Options {
        cwd: root.canonicalize().unwrap(),
        config: Some(root.join("tuse.lua").canonicalize().unwrap()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    assert!(s.config().prompts.contains_key("openspec"));
    s.close();
    common::cleanup(&dir);
}
