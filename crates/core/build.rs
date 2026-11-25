use std::fmt::Write;
use std::path::{Path, PathBuf};

const TOPICS: &[&str] = &["usage", "connect", "config", "lua", "session", "json"];

fn summary(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .skip_while(|l| l.starts_with("# ") || l.trim().is_empty())
        .take_while(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let docs = root.join("docs");
    let readme = root.join("README.md");
    println!("cargo:rerun-if-changed={}", docs.display());
    println!("cargo:rerun-if-changed={}", readme.display());
    for entry in std::fs::read_dir(&docs).unwrap().flatten() {
        let name = entry.file_name().into_string().unwrap_or_default();
        if let Some(stem) = name.strip_suffix(".md") {
            assert!(TOPICS.contains(&stem), "docs/{name} is not a docs topic");
        }
    }
    let mut out = String::from("pub(crate) const TOPICS: &[(&str, &str, &str)] = &[\n");
    writeln!(
        out,
        "    (\"readme\", \"Overview, install, and quick start\", include_str!({:?})),",
        readme.display().to_string()
    )
    .unwrap();
    for name in TOPICS {
        println!(
            "cargo:rerun-if-changed={}",
            docs.join(format!("{name}.md")).display()
        );
        let path = docs.join(format!("{name}.md"));
        writeln!(
            out,
            "    ({name:?}, {:?}, include_str!({:?})),",
            summary(&path),
            path.display().to_string()
        )
        .unwrap();
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("topics.rs");
    std::fs::write(dest, out).unwrap();
}
