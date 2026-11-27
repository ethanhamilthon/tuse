const TEMPLATE: &str = r#"return {
  project = {
    systems = {
    },
  },
}
"#;

pub fn run(cwd: &std::path::Path) -> Result<(), String> {
    let path = cwd.join("tuse.lua");
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    std::fs::write(&path, TEMPLATE).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("created {}", path.display());
    Ok(())
}
