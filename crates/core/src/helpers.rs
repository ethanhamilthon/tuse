use crate::{Event, Session, host::last_line};
use std::time::{Duration, Instant};
use tuse_config::Config;
use tuse_py::{Ctx, NEVER, Python};

pub fn install(s: &Session, py: &mut Python, config: &Config) {
    let ctx = Ctx::new(
        &NEVER,
        Instant::now() + Duration::from_secs(config.settings.hook_timeout.max(1)),
    );
    for (name, f) in &config.functions {
        let mut src = format!("def {name}({}):\n", f.args);
        for line in f.run.lines() {
            src.push_str("    ");
            src.push_str(line);
            src.push('\n');
        }
        let out = py.run(&src, &ctx, vec![]);
        if !out.ok {
            s.emit(Event::Notice(format!(
                "function {name}: {}",
                last_line(&out.text)
            )));
        }
    }
}

pub fn listing(config: &Config, full: bool) -> String {
    if config.functions.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n# Your functions in `python`");
    for (name, f) in config
        .functions
        .iter()
        .take(if full { usize::MAX } else { 30 })
    {
        let desc: String = f
            .description
            .chars()
            .take(if full { usize::MAX } else { 200 })
            .collect();
        out.push_str(&format!("\n- `{name}({})`: {desc}", f.args));
    }
    if !full && config.functions.len() > 30 {
        out.push_str(&format!(
            "\nand {} more, see `docs(\"functions\")`",
            config.functions.len() - 30
        ));
    }
    out
}
