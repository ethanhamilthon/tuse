//! `tuse models`: manage the default model/effort and saved model metadata in SQLite.

use crate::args::Args;
use std::path::PathBuf;
use tuse_config::{EFFORTS, ModelDef, full_model_name};

const USAGE: &str = "tuse models [list]
tuse models use [MODEL] [--effort EFFORT]
tuse models set NAME [--display TEXT] [--window N] [--compact-at N|auto]
                     [--price-in F] [--price-out F] [--price-cache F] [--price-cache-write F]
                     [--clear-prices]
tuse models remove NAME
tuse models import [PATH]";

pub fn run(raw: &[String]) -> Result<(), String> {
    let sub = raw.first().map(String::as_str).unwrap_or("list");
    let rest = raw.get(1..).unwrap_or_default();
    match sub {
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        "list" => list(rest),
        "use" => select(rest),
        "set" => set(rest),
        "remove" | "rm" => remove(rest),
        "import" => import(rest),
        other => Err(format!("unknown subcommand `{other}`\n\n{USAGE}")),
    }
}

fn list(rest: &[String]) -> Result<(), String> {
    if !rest.is_empty() {
        return Err(format!("usage: tuse models [list]\n\n{USAGE}"));
    }
    let model = tuse_core::default_model();
    let effort = tuse_core::default_effort();
    println!(
        "Default: {} (effort: {effort})",
        if model.is_empty() { "(none)" } else { &model }
    );
    let defs = tuse_core::models();
    if defs.is_empty() {
        println!("No model metadata saved.");
        return Ok(());
    }
    for m in defs {
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

fn select(rest: &[String]) -> Result<(), String> {
    let args = Args::parse(rest, &["--effort", "-e"])?;
    if let Some(f) = args.unknown(&["--effort", "-e"]) {
        return Err(format!("unknown flag {f}\n\n{USAGE}"));
    }
    if args.positional.len() > 1 {
        return Err(format!(
            "usage: tuse models use [MODEL] [--effort EFFORT]\n\n{USAGE}"
        ));
    }
    let model = args.positional.first().cloned();
    let effort = args.value(&["--effort", "-e"]);
    if model.is_none() && effort.is_none() {
        return Err(format!("specify MODEL and/or --effort\n\n{USAGE}"));
    }
    if let Some(e) = &effort {
        if !EFFORTS.contains(&e.as_str()) {
            return Err(format!("`--effort` must be one of {}", EFFORTS.join(", ")));
        }
        tuse_core::set_state("effort", e)?;
    }
    if let Some(m) = &model {
        let providers = tuse_core::providers();
        let full = full_model_name(m, "", &providers);
        tuse_core::set_state("model", &full)?;
    }
    println!(
        "Default: {} (effort: {})",
        tuse_core::default_model(),
        tuse_core::default_effort()
    );
    Ok(())
}

fn set(rest: &[String]) -> Result<(), String> {
    let flags = &[
        "--display",
        "--window",
        "--compact-at",
        "--price-in",
        "--price-out",
        "--price-cache",
        "--price-cache-write",
        "--clear-prices",
    ];
    let valued = &flags[..flags.len() - 1];
    let args = Args::parse(rest, valued)?;
    if let Some(f) = args.unknown(flags) {
        return Err(format!("unknown flag {f}\n\n{USAGE}"));
    }
    let [name] = args.positional.as_slice() else {
        return Err(format!("usage: tuse models set NAME [options]\n\n{USAGE}"));
    };
    let mut def = tuse_core::model_def(name).unwrap_or_else(|| ModelDef {
        name: name.clone(),
        ..Default::default()
    });
    if let Some(d) = args.value(&["--display"]) {
        def.display = d;
    }
    if let Some(w) = args.value(&["--window"]) {
        def.window = w
            .parse::<u64>()
            .map_err(|_| format!("--window must be a non-negative integer, got `{w}`"))?;
    }
    if let Some(c) = args.value(&["--compact-at"]) {
        def.compact_at = if c.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(
                c.parse::<u64>()
                    .map_err(|_| format!("--compact-at must be an integer or `auto`, got `{c}`"))?,
            )
        };
    }
    let clear = rest.iter().any(|s| s == "--clear-prices");
    if clear {
        def.prices = None;
    }
    let p_in = parse_opt_f64(args.value(&["--price-in"]), "--price-in")?;
    let p_out = parse_opt_f64(args.value(&["--price-out"]), "--price-out")?;
    let p_cache = parse_opt_f64(args.value(&["--price-cache"]), "--price-cache")?;
    let p_write = parse_opt_f64(args.value(&["--price-cache-write"]), "--price-cache-write")?;
    if p_in.is_some() || p_out.is_some() || p_cache.is_some() || p_write.is_some() {
        let mut pr = def.prices.unwrap_or_default();
        if let Some(v) = p_in {
            pr.input = v;
        }
        if let Some(v) = p_out {
            pr.output = v;
        }
        if let Some(v) = p_cache {
            pr.cache_read = v;
        }
        if let Some(v) = p_write {
            pr.cache_write = v;
        }
        def.prices = Some(pr);
    }
    tuse_core::set_model_def(&def)?;
    println!("Saved model metadata for `{}`.", def.name);
    Ok(())
}

fn parse_opt_f64(v: Option<String>, flag: &str) -> Result<Option<f64>, String> {
    let Some(s) = v else { return Ok(None) };
    let n = s
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.0)
        .ok_or_else(|| format!("{flag} must be a non-negative number, got `{s}`"))?;
    Ok(Some(n))
}

fn remove(rest: &[String]) -> Result<(), String> {
    let [name] = rest else {
        return Err(format!("usage: tuse models remove NAME\n\n{USAGE}"));
    };
    if !tuse_core::remove_model_def(name)? {
        return Err(format!("no saved model metadata for `{name}`"));
    }
    println!("Removed `{name}`.");
    Ok(())
}

fn import(rest: &[String]) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let path: PathBuf = match rest {
        [] => tuse_config::find(None, &cwd)
            .ok_or("no config file found; pass a path: `tuse models import PATH`")?,
        [p] => cwd.join(p),
        _ => return Err(format!("usage: tuse models import [PATH]\n\n{USAGE}")),
    };
    let report = tuse_core::import_legacy_models(&path)?;
    let mut any = false;
    if let Some(m) = &report.model {
        println!("Imported default model: {m}");
        any = true;
    }
    if let Some(e) = &report.effort {
        println!("Imported default effort: {e}");
        any = true;
    }
    if !report.models.is_empty() {
        println!(
            "Imported {} model(s): {}",
            report.models.len(),
            report.models.join(", ")
        );
        any = true;
    }
    if any {
        println!(
            "You can now remove `model`, `provider`, `effort`, and `models` from {}.",
            path.display()
        );
    } else {
        println!("No legacy model settings found in {}.", path.display());
    }
    Ok(())
}
