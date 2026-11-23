use super::*;
use crate::links::{
    LinkTarget, highlight, intern, openable_url, resolve_markdown_target, scan, target,
};
use crate::panel::Panel;
use crate::style::Palette;

fn pieces(t: &str, cwd: &Path) -> Vec<(String, Option<LinkTarget>)> {
    split(t, cwd)
        .into_iter()
        .map(|(s, i)| (s, i.and_then(target)))
        .collect()
}

#[test]
fn finds_bare_urls_and_drops_trailing_punctuation() {
    let p = pieces(
        "see https://a.dev/x?q=1, and (http://b.io/p_(1)).",
        Path::new(""),
    );
    assert_eq!(
        p,
        [
            ("see ".into(), None),
            (
                "https://a.dev/x?q=1".into(),
                Some(LinkTarget::Url("https://a.dev/x?q=1".into()))
            ),
            (", and (".into(), None),
            (
                "http://b.io/p_(1)".into(),
                Some(LinkTarget::Url("http://b.io/p_(1)".into()))
            ),
            (").".into(), None),
        ]
    );
    assert_eq!(
        pieces("no link here", Path::new("")),
        [("no link here".into(), None)]
    );
    assert_eq!(
        pieces("xhttps://a.dev https://", Path::new("")),
        [("xhttps://a.dev https://".into(), None)]
    );
}

#[test]
fn detects_relative_absolute_and_line_suffixed_paths() {
    let dir = std::env::temp_dir().join(format!("tuse-links-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let file = dir.join("sub/note.md");
    std::fs::write(&file, "hi").unwrap();
    let canon = file.canonicalize().unwrap();
    let text = format!(
        "open sub/note.md:42:3 and ./sub/note.md and ({}) not/real.md",
        file.display()
    );
    let got = pieces(&text, &dir);
    let paths: Vec<_> = got
        .iter()
        .filter_map(|(s, t)| match t {
            Some(LinkTarget::Path(p)) => Some((s.as_str(), p.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        paths,
        [
            ("sub/note.md:42:3", canon.clone()),
            ("./sub/note.md", canon.clone()),
            (file.to_str().unwrap(), canon.clone()),
        ]
    );
    assert_eq!(
        resolve_markdown_target("./sub/note.md#sec", &dir),
        Some(LinkTarget::Path(dir.join("./sub/note.md")))
    );
    assert!(openable_url("HTTPS://x.dev") && openable_url("mailto:a@b.c"));
    assert!(!openable_url("file:///etc/passwd") && !openable_url("-a Calculator"));
    assert_eq!(
        intern(LinkTarget::Url("https://same.example".into())),
        intern_url("https://same.example")
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn wrapped_links_and_views_share_link_id_and_hit_testing() {
    let dir = std::env::temp_dir().join(format!("tuse-links-wrap-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("deep/folder/name")).unwrap();
    let file = dir.join("deep/folder/name/example.md");
    std::fs::write(&file, "hello").unwrap();
    let canon = file.canonicalize().unwrap();
    let mut p = Palette::new(&Default::default(), &Default::default());
    p.cwd = dir.clone();

    // Wrapped relative path across narrow width keeps one link ID on every wrapped line.
    let mut wrapped = crate::markdown::render(
        "Check deep/folder/name/example.md and https://example.com/very/long/url/path",
        20,
        &p,
    );
    let boxes = scan(&wrapped, 0);
    let path_boxes: Vec<_> = boxes
        .iter()
        .filter(|b| target(b.id) == Some(LinkTarget::Path(canon.clone())))
        .collect();
    assert!(
        path_boxes.len() >= 2,
        "expected wrapped path boxes: {boxes:?}"
    );
    let url_id = boxes
        .iter()
        .find(|b| {
            matches!(
                target(b.id),
                Some(LinkTarget::Url(ref u)) if u == "https://example.com/very/long/url/path"
            )
        })
        .expect("url box")
        .id;
    let url_boxes: Vec<_> = boxes.iter().filter(|b| b.id == url_id).collect();
    assert!(
        url_boxes.len() >= 2,
        "expected wrapped url boxes: {boxes:?}"
    );
    highlight(&mut wrapped, Some(url_id), p.accent);
    assert!(
        wrapped
            .iter()
            .flatten()
            .filter(|s| s.style.link == Some(url_id))
            .all(|s| s.style.bold && s.style.fg == p.accent)
    );

    // Custom markdown block, tool output, diff path, and trace/details panel all linkify.
    let md_block = crate::render::ui_markdown("See [doc](deep/folder/name/example.md)", 60, &p);
    assert!(!scan(&md_block, 0).is_empty());

    let tool_lines = crate::tool_view::tool(
        "read('deep/folder/name/example.md')",
        Some("deep/folder/name/example.md:1"),
        Some(0),
        &[tuse_core::FileDiff {
            path: "deep/folder/name/example.md".into(),
            diff: "+ hi".into(),
        }],
        60,
        &p,
    );
    let tool_boxes = scan(&tool_lines, 0);
    assert!(
        tool_boxes
            .iter()
            .filter(|b| target(b.id) == Some(LinkTarget::Path(canon.clone())))
            .count()
            >= 2,
        "expected links in tool output & diff header: {tool_boxes:?}"
    );

    let mut trace_panel = Panel::new("opened deep/folder/name/example.md");
    let panel_lines = trace_panel.lines(60, 10, &p);
    assert!(
        scan(&panel_lines, 0)
            .iter()
            .any(|b| target(b.id) == Some(LinkTarget::Path(canon.clone())))
    );
    std::fs::remove_dir_all(dir).ok();
}
