use std::fmt::Write;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let docs = root.join("docs");
    println!("cargo:rerun-if-changed={}", docs.display());
    println!(
        "cargo:rerun-if-changed={}",
        root.join("README.md").display()
    );
    let index = std::fs::read_to_string(docs.join("index.md")).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&docs)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().into_string().ok()?;
            let stem = n.strip_suffix(".md")?.to_string();
            (stem != "index").then_some(stem)
        })
        .collect();
    names.sort();
    let mut out = String::from("pub(crate) const TOPICS: &[(&str, &str, &str)] = &[\n");
    let readme = root.join("README.md");
    writeln!(
        out,
        "    (\"README.md\", \"Concise overview, install, quick start, and links to docs\", include_str!({:?})),",
        readme.display().to_string()
    )
    .unwrap();
    for name in &names {
        let link = format!("]({name}.md)");
        let summary = index
            .lines()
            .find(|l| l.contains(&link))
            .and_then(|l| l.split_once(" — ").map(|(_, s)| s.trim().to_string()))
            .unwrap_or_default();
        let path = docs.join(format!("{name}.md"));
        writeln!(
            out,
            "    ({name:?}, {summary:?}, include_str!({:?})),",
            path.display().to_string()
        )
        .unwrap();
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("topics.rs");
    std::fs::write(dest, out).unwrap();
}
