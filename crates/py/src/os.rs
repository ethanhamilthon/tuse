use crate::ctx::Ctx;
use crate::host::Host;
use monty_fs::{MountCallOutcome, MountTable};
use monty_types::{ExtFunctionResult, IMPORT_CALL, MontyObject, OsFunctionCall, find_module_file};
use std::time::Duration;

/// Answers the interpreter's OS-level requests: files through the mounts (the
/// whole filesystem, read-write), the environment, `import` of `.py` files from
/// the config folder and the working directory, and sleeps.
pub fn handle(
    mounts: &mut MountTable,
    host: &dyn Host,
    ctx: &Ctx,
    call: OsFunctionCall,
) -> ExtFunctionResult {
    match call {
        OsFunctionCall::Getenv(a) => match lookup(host, &a.key) {
            Some(v) => ExtFunctionResult::Return(MontyObject::string(v)),
            None => ExtFunctionResult::Return(a.default),
        },
        OsFunctionCall::GetEnviron => {
            let mut all: Vec<(String, String)> = std::env::vars().collect();
            all.extend(host.env());
            all.sort();
            all.dedup_by(|b, a| {
                a.0 == b.0 && {
                    a.1 = b.1.clone();
                    true
                }
            });
            let pairs = all
                .into_iter()
                .map(|(k, v)| (MontyObject::string(k), MontyObject::string(v)));
            ExtFunctionResult::Return(MontyObject::dict(pairs))
        }
        OsFunctionCall::Import(mut a) => {
            let mut dirs: Vec<String> = host
                .module_dirs()
                .iter()
                .map(|d| d.display().to_string())
                .collect();
            dirs.append(&mut a.search_path);
            a.search_path = dirs;
            find_module_file(&a, |p| mounts.read_module(p))
                .unwrap_or_else(|| ExtFunctionResult::NotFound(IMPORT_CALL.to_owned()))
        }
        OsFunctionCall::Sleep(d) | OsFunctionCall::SystemSleep(d) => {
            sleep(d, ctx);
            ExtFunctionResult::Return(MontyObject::none())
        }
        call => match mounts.handle_os_call(call) {
            MountCallOutcome::Handled(Ok(v)) => ExtFunctionResult::Return(v),
            MountCallOutcome::Handled(Err(e)) => ExtFunctionResult::Error(e.into_exception()),
            MountCallOutcome::NotHandled(c) => c.no_handler_result(),
        },
    }
}

fn lookup(host: &dyn Host, key: &str) -> Option<String> {
    host.env()
        .into_iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .or_else(|| std::env::var(key).ok())
}

/// Sleeps in short slices so a cancel or the deadline ends it.
fn sleep(total: Duration, ctx: &Ctx) {
    let end = std::time::Instant::now() + total;
    while !ctx.stopped() && std::time::Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
}
