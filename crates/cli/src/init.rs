//! `tuse init`: a `./tuse.lua` that extends the global config.

const TEMPLATE: &str = r#"-- Project config: starts from the global one and changes what this project needs.
local ok, g = pcall(require, "global")
g = ok and g or {}

-- g.systems = g.systems or {}
-- table.insert(g.systems, "Project rules go here.")

return g
"#;

pub fn run(cwd: &std::path::Path) -> Result<(), String> {
    let path = cwd.join("tuse.lua");
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    std::fs::write(&path, TEMPLATE).map_err(|e| format!("{}: {e}", path.display()))?;
    match tuse_config::global_path() {
        Some(g) => println!("created {} (extends {})", path.display(), g.display()),
        None => println!(
            "created {}; there is no global config yet, so it starts empty",
            path.display()
        ),
    }
    Ok(())
}
