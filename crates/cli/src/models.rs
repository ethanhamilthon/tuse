use std::path::PathBuf;

pub fn run(cwd: PathBuf, config: Option<PathBuf>) -> Result<(), String> {
    let defs = tuse_core::configured_models(cwd, config)?;
    let model = tuse_core::Prefs::model();
    let effort = tuse_core::Prefs::effort();
    println!(
        "Default: {} (effort: {effort})",
        if model.is_empty() { "(none)" } else { &model }
    );
    let defs: std::collections::BTreeMap<_, _> =
        defs.into_iter().map(|m| (m.name.clone(), m)).collect();
    if defs.is_empty() {
        println!("No models configured.");
        return Ok(());
    }
    for m in defs.into_values() {
        let mut parts = vec![];
        if !m.display.is_empty() {
            parts.push(m.display);
        }
        if m.window > 0 {
            parts.push(format!("{} ctx", tuse_core::tokens(m.window)));
        }
        if let Some(c) = m.compact_at {
            parts.push(if c == 0 {
                "compact off".into()
            } else {
                format!("compact {}", tuse_core::tokens(c))
            });
        }
        if let Some(p) = m.prices {
            parts.push(format!(
                "${}/${} (cache ${}/${}) per 1M",
                p.input, p.output, p.cache_read, p.cache_write
            ));
        }
        println!("{}\t{}", m.name, parts.join(" · "));
    }
    Ok(())
}
