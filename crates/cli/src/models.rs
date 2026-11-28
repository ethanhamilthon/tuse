use std::path::PathBuf;

pub fn run(cwd: PathBuf) -> Result<(), String> {
    let defs = tuse_core::configured_models(cwd)?;
    let model = tuse_core::Prefs::model();
    let cfg = tuse_config::Config {
        models: defs.clone(),
        ..Default::default()
    };
    let effort = tuse_core::Prefs::effort_for(&cfg, &model);
    let shown_effort = if effort.is_empty() { "none" } else { &effort };
    println!(
        "Default: {} (effort: {shown_effort})",
        if model.is_empty() { "(none)" } else { &model }
    );
    if defs.is_empty() {
        println!("No models configured.");
        return Ok(());
    }
    for (name, m) in defs {
        let mut parts = vec![];
        if !m.display.is_empty() {
            parts.push(m.display);
        }
        if m.window > 0 {
            parts.push(format!("{} ctx", tuse_core::tokens(m.window)));
        }
        if let Some(effs) = m.efforts {
            parts.push(if effs.is_empty() {
                "efforts=none".into()
            } else {
                format!("efforts={}", effs.join(","))
            });
        }
        if let Some(p) = m.prices {
            parts.push(format!(
                "${}/${} (cache ${}/${}) per 1M",
                p.input, p.output, p.cache_read, p.cache_write
            ));
        }
        println!("{name}\t{}", parts.join(" · "));
    }
    Ok(())
}
