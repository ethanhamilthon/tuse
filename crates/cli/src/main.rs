mod args;
mod models;
mod update;

use args::Args;
use std::path::PathBuf;
use tuse_core::{LoadFlags, Options, Providers, Session};

const USAGE: &str = "tuse - minimal AI coding agent

USAGE
  tuse [-r|--resume ID] [--no-global] [--no-plugins]
                                                 open the TUI; -r ID opens a saved session
                                                 (its ID is printed to stderr on exit)
  tuse json [-r|--resume ID] [--no-global] [--no-plugins]
                                                 a JSON-RPC 2.0 session over stdin/stdout,
                                                 one message per line (see docs/json.md)
  tuse providers [--json]                        list saved providers
  tuse models                 list configured model metadata and the
                                                 saved default
  tuse update                                    check for updates and update the binary
  tuse rollback                                  restore the previous binary backup
  tuse version | -V|--version                    print the version
  tuse help [COMMAND] | -h|--help                this help; `tuse COMMAND -h` shows one command

Global config lookup: $TUSE_GLOBAL_CONFIG (if set), else ~/.config/tuse.lua or ~/.config/tuse/init.lua;
installed plugins from the plugins/ subdirectory of the Tuse storage directory.
Storage: $TUSE_HOME (if set and non-empty), else ~/.tuse.";

struct Cmd {
    usage: &'static str,
    valued: &'static [&'static str],
    plain: &'static [&'static str],
    args: usize,
}

const SESSION: &[&str] = &["--resume", "-r"];
const LOAD_FLAGS: &[&str] = &["--no-global", "--no-plugins"];

fn spec(sub: &str) -> Option<Cmd> {
    let cmd = |usage, valued, plain, args| Cmd {
        usage,
        valued,
        plain,
        args,
    };
    Some(match sub {
        "" => cmd(
            "tuse [-r|--resume ID] [--no-global] [--no-plugins]",
            SESSION,
            LOAD_FLAGS,
            0,
        ),
        "json" => cmd(
            "tuse json [-r|--resume ID] [--no-global] [--no-plugins]",
            SESSION,
            LOAD_FLAGS,
            0,
        ),
        "providers" => cmd("tuse providers [--json]", &[], &["--json"], 0),
        "models" => cmd("tuse models", &[], &[], 0),
        "update" => cmd("tuse update", &[], &[], 0),
        "rollback" => cmd("tuse rollback", &[], &[], 0),
        "version" => cmd("tuse version", &[], &[], 0),
        "help" => cmd("tuse help [COMMAND]", &[], &[], 0),
        _ => return None,
    })
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(raw) {
        eprintln!("tuse: {e}");
        std::process::exit(1);
    }
}

fn run(raw: Vec<String>) -> Result<(), String> {
    let first = raw.first().map(String::as_str).unwrap_or("");
    let (sub, rest) = match first {
        "--version" | "-V" => ("version", &raw[1..]),
        "--help" | "-h" => {
            println!("{USAGE}");
            return Ok(());
        }
        s if s.starts_with('-') => ("", &raw[..]),
        s => (s, raw.get(1..).unwrap_or_default()),
    };
    let Some(cmd) = spec(sub) else {
        return Err(format!("unknown command `{sub}`\n\n{USAGE}"));
    };
    if sub == "help" {
        return run_help(rest);
    }
    let help = rest
        .iter()
        .take_while(|s| *s != "--")
        .any(|s| s == "-h" || s == "--help");
    if help {
        println!("{}", if sub.is_empty() { USAGE } else { cmd.usage });
        return Ok(());
    }
    let a = Args::parse(rest, cmd.valued, cmd.plain)?;
    let known: Vec<&str> = cmd.valued.iter().chain(cmd.plain).copied().collect();
    if let Some(f) = a.unknown(&known) {
        return Err(format!("unknown flag {f}\nusage: {}", cmd.usage));
    }
    a.expect_args(cmd.args)
        .map_err(|msg| format!("{msg}\nusage: {}", cmd.usage))?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    match sub {
        "providers" => providers_list(a.has(&["--json"])),
        "models" => models::run(cwd),
        "json" => json(&a, cwd),
        "update" => update::update(),
        "rollback" => update::rollback(),
        "version" => {
            println!("tuse {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => start(&a, cwd),
    }
}

fn providers_list(as_json: bool) -> Result<(), String> {
    if as_json {
        println!("{}", Providers::json()?);
    } else {
        for p in Providers::views()? {
            println!("{}\t{}", p.name, p.base);
        }
    }
    Ok(())
}

fn run_help(rest: &[String]) -> Result<(), String> {
    if rest.len() == 1 && matches!(rest[0].as_str(), "-h" | "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    let a = Args::parse(rest, &[], &[])?;
    if let Some(f) = a.unknown(&[]) {
        return Err(format!("unknown flag {f}\nusage: tuse help [COMMAND]"));
    }
    a.expect_args_range(0, 1)
        .map_err(|msg| format!("{msg}\nusage: tuse help [COMMAND]"))?;
    match a.opt_arg(0) {
        None => println!("{USAGE}"),
        Some(target) => match spec(target).filter(|_| !target.is_empty()) {
            Some(c) => println!("{}", c.usage),
            None => {
                return Err(format!(
                    "unknown command `{target}`\nusage: tuse help [COMMAND]"
                ));
            }
        },
    }
    Ok(())
}

fn open_session(
    a: &Args,
    cwd: PathBuf,
    mode: tuse_core::Mode,
) -> Result<std::sync::Arc<Session>, String> {
    Session::open(Options {
        cwd,
        mode,
        resume: a.value(&["--resume", "-r"]),
        start: tuse_core::Start::Launch,
        flags: LoadFlags {
            no_global: a.has(&["--no-global"]),
            no_plugins: a.has(&["--no-plugins"]),
        },
    })
}

fn start(a: &Args, cwd: PathBuf) -> Result<(), String> {
    let session = open_session(a, cwd, tuse_core::Mode::Tui)?;
    let id = tuse_tui::run(session).map_err(|e| e.to_string())?;
    eprintln!("tuse: session {id}");
    Ok(())
}

fn json(a: &Args, cwd: PathBuf) -> Result<(), String> {
    let session = open_session(a, cwd, tuse_core::Mode::Json)?;
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for sig in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(sig, stop.clone()).map_err(|e| e.to_string())?;
    }
    let stdin = std::io::BufReader::new(std::io::stdin());
    tuse_rpc::serve(session, stdin, tuse_rpc::Out::new(std::io::stdout()), stop);
    Ok(())
}

#[cfg(test)]
mod tests {
    const ALL: &[&str] = &[
        "",
        "json",
        "providers",
        "models",
        "update",
        "rollback",
        "version",
        "help",
    ];

    #[test]
    fn help_and_docs_list_every_command() {
        let docs = tuse_core::docs(Some("usage"));
        for sub in ALL {
            let cmd = super::spec(sub).unwrap();
            let first = cmd.usage.lines().next().unwrap();
            let name = first.split(" [").next().unwrap();
            let name = name.split(" --").next().unwrap();
            assert!(super::USAGE.contains(name), "help misses {name}");
            assert!(docs.contains(&format!("`{name}")), "docs misses {name}");
            if cmd.valued.contains(&"--config") {
                assert!(cmd.usage.contains("[-c|--config PATH]"), "{}", cmd.usage);
            }
        }
    }
}
