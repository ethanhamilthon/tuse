//! `tuse init`: a `./tuse.lua` that extends the global config.

const TEMPLATE: &str = r#"-- Project config: starts from the global one and changes what this project needs.
-- TUSE_GLOBAL_CONFIG is set only when the global config exists.
local g = os.getenv("TUSE_GLOBAL_CONFIG") and require("global") or {}

-- g.model = "provider/model"
-- g.systems = g.systems or {}
-- table.insert(g.systems, "Project rules go here.")

return g
"#;

pub fn run(rest: &[String]) -> Result<(), String> {
    if let Some(a) = rest.first() {
        return Err(format!("unexpected argument `{a}`\nusage: tuse init"));
    }
    let path = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("tuse.lua");
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
