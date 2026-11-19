mod diff;
mod edit;
mod fs;
mod grep;
mod read;
mod write;

use std::path::Path;
use tuse_config::Config;
use tuse_py::{Args, Ctx, ExcType, Fail, MontyObject};

pub use fs::resolve;

fn io_fail(path: &Path, e: &std::io::Error) -> Fail {
    let kind = match e.kind() {
        std::io::ErrorKind::NotFound => ExcType::FileNotFoundError,
        std::io::ErrorKind::PermissionDenied => ExcType::PermissionError,
        _ => ExcType::OSError,
    };
    Fail::of(kind, format!("{}: {e}", path.display()))
}

/// Runs `read`, `write`, `edit` or `grep`. Changed files are recorded in `ctx`
/// with their diff (shown after the call), images are attached to it.
pub fn call(
    name: &str,
    a: &Args,
    ctx: &Ctx,
    cwd: &Path,
    config: &Config,
) -> Result<MontyObject, Fail> {
    let path = |i, key| a.str(i, key).map(|p| resolve(&p, cwd));
    match name {
        "read" => {
            let path = path(0, "path")?;
            let offset = a.opt_int(1, "offset")?.map_or(1, |n| n.max(1) as usize);
            let limit = a
                .opt_int(2, "limit")?
                .map_or(read::DEFAULT_LIMIT, |n| n.max(1) as usize);
            let r = read::run(
                &path,
                offset,
                limit,
                config.settings.max_image_bytes,
                config.settings.max_output,
            )?;
            if let Some(url) = r.image {
                ctx.add_image(url);
            }
            Ok(MontyObject::string(r.text))
        }
        "write" => {
            let (path, content) = (path(0, "path")?, a.str(1, "content")?);
            let (msg, diff) = write::run(&path, &content)?;
            ctx.add_diff(&path.display().to_string(), diff);
            Ok(MontyObject::string(msg))
        }
        "edit" => {
            let path = path(0, "path")?;
            let (old, new) = (a.str(1, "old")?, a.str(2, "new")?);
            let all = a.bool(3, "replace_all", false)?;
            let (msg, diff) = edit::run(&path, &old, &new, all)?;
            ctx.add_diff(&path.display().to_string(), diff);
            Ok(MontyObject::string(msg))
        }
        "grep" => grep::call(a, ctx, cwd),
        _ => Err(Fail::runtime(format!("unknown function {name}"))),
    }
}
