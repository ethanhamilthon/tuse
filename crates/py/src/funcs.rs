use crate::ctx::Ctx;
use crate::fail::{Args, Fail};
use crate::host::Host;
use crate::sh;
use crate::value::{opt_int, record};
use monty_types::{CallArgs, MontyObject};
use std::path::PathBuf;
use std::time::Duration;

/// `sh` plus whatever the host provides.
pub fn names(host: &dyn Host) -> Vec<&'static str> {
    let mut v = vec!["sh"];
    v.extend(host.functions());
    v
}

pub fn call(host: &dyn Host, name: &str, args: &CallArgs, ctx: &Ctx) -> Result<MontyObject, Fail> {
    let a = Args(args);
    host.status(name, &a);
    if name != "sh" {
        return host.call(name, &a, ctx);
    }
    let cmd = a.str(0, "cmd")?;
    let timeout = a
        .opt_int(1, "timeout")?
        .map(|s| Duration::from_secs(s.max(1) as u64))
        .map_or(ctx.remaining(), |t| t.min(ctx.remaining()));
    let cwd = a
        .opt_str(2, "cwd")?
        .map_or_else(|| host.cwd(), |c| host.cwd().join(PathBuf::from(c)));
    let mut env = host.env();
    env.extend(a.str_map(3, "env")?);
    let adopt = |a| host.adopt(a);
    let r = sh::run(
        &cmd,
        &cwd,
        &env,
        timeout,
        ctx.cancel,
        ctx.adopt.then_some(&adopt as sh::Adopter),
    );
    if r.task.is_some() {
        ctx.grace();
    }
    Ok(record(
        "Sh",
        vec![
            ("out", MontyObject::string(r.out)),
            ("code", opt_int(r.code)),
            ("ok", MontyObject::bool(r.code == Some(0))),
            ("timed_out", MontyObject::bool(r.timed_out)),
            (
                "task",
                r.task
                    .as_deref()
                    .map_or_else(MontyObject::none, MontyObject::string),
            ),
        ],
    ))
}
