mod args;
mod connect;
mod init;
mod models;
mod plugins;
mod update;

use args::Args;
use std::path::PathBuf;

use tuse_core::{Options, Session};

const USAGE: &str = "tuse - minimal AI coding agent

USAGE
  tuse [-c|--config PATH] [-r|--resume ID]       open the TUI; -r ID opens a saved session
                                                 (its ID is printed to stderr on exit)
  tuse json [-c|--config PATH] [-r|--resume ID]  a JSON-RPC 2.0 session over stdin/stdout,
                                                 one message per line (see docs/json.md)
  tuse connect --base URL (--key '$ENV' | --key-stdin) --name NAME [--api TYPE] [--header HEADER]... [--replace]
                                                 verify and save an API provider
  tuse disconnect NAME                           remove a provider
  tuse providers [--json]                        list the providers
  tuse models [-c|--config PATH]                 list configured model metadata and the
                                                 saved default
  tuse plugins <add|list|delete|print|update>    manage installed Lua plugins
  tuse init                                      create a starter namespaced ./tuse.lua
  tuse update                                    update the binary
  tuse version | -V|--version                    print the version
  tuse help [COMMAND] | -h|--help                this help; `tuse COMMAND -h` shows one command

Config lookup: -c|--config PATH, else ./tuse.lua as primary local config; global config
from $TUSE_GLOBAL_CONFIG (if set), else ~/.config/tuse.lua or ~/.config/tuse/init.lua;
installed plugins from ~/.tuse/plugins.";

struct Cmd {
    usage: &'static str,
    valued: &'static [&'static str],
    plain: &'static [&'static str],
    args: usize,
}

const CONFIG: &[&str] = &["--config", "-c"];
const SESSION: &[&str] = &["--config", "-c", "--resume", "-r"];

fn spec(sub: &str) -> Option<Cmd> {
    let cmd = |usage, valued, args| Cmd {
        usage,
        valued,
        plain: &[],
        args,
    };
    Some(match sub {
        "" => cmd("tuse [-c|--config PATH] [-r|--resume ID]", SESSION, 0),
        "json" => cmd("tuse json [-c|--config PATH] [-r|--resume ID]", SESSION, 0),
        "connect" => Cmd {
            usage: "tuse connect --base URL (--key '$ENV' | --key-stdin) --name NAME [--api TYPE] [--header HEADER]... [--replace]",
            valued: &["--base", "--key", "--name", "--api", "--header"],
            plain: &["--replace", "--key-stdin"],
            args: 0,
        },
        "disconnect" => cmd("tuse disconnect NAME", &[], 1),
        "providers" => Cmd {
            usage: "tuse providers [--json]",
            valued: &[],
            plain: &["--json"],
            args: 0,
        },
        "models" => cmd("tuse models [-c|--config PATH]", CONFIG, 0),
        "plugins" => cmd(plugins::USAGE, &[], 0),
        "init" => cmd("tuse init", &[], 0),
        "update" => cmd("tuse update", &[], 0),
        "version" => cmd("tuse version", &[], 0),
        "help" => cmd("tuse help [COMMAND]", &[], 0),
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
    if sub == "plugins" {
        return plugins::run(rest);
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
    let config = tuse_core::config_arg(&cwd, a.value(CONFIG).as_deref());
    match sub {
        "connect" => connect::run(&a),
        "disconnect" => connect::remove(a.arg(0)),
        "providers" => connect::list(a.has(&["--json"])),
        "models" => models::run(cwd, config),
        "json" => json(&a, cwd, config),
        "init" => init::run(&cwd),
        "update" => update::update(),
        "version" => {
            println!("tuse {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => start(&a, cwd, config),
    }
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
    config: Option<PathBuf>,
    mode: tuse_core::Mode,
) -> Result<std::sync::Arc<Session>, String> {
    Session::open(Options {
        cwd,
        config,
        mode,
        resume: a.value(&["--resume", "-r"]),
        start: tuse_core::Start::Launch,
    })
}

fn start(a: &Args, cwd: PathBuf, config: Option<PathBuf>) -> Result<(), String> {
    let session = open_session(a, cwd, config.clone(), tuse_core::Mode::Tui)?;
    let id = tuse_tui::run(session, config).map_err(|e| e.to_string())?;
    eprintln!("tuse: session {id}");
    Ok(())
}

fn json(a: &Args, cwd: PathBuf, config: Option<PathBuf>) -> Result<(), String> {
    let session = open_session(a, cwd, config, tuse_core::Mode::Json)?;
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
        "connect",
        "disconnect",
        "providers",
        "models",
        "plugins",
        "init",
        "update",
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
            let name = name.split(" NAME").next().unwrap();
            let name = name.split(" <").next().unwrap();
            assert!(super::USAGE.contains(name), "help misses {name}");
            assert!(docs.contains(&format!("`{name}")), "docs misses {name}");
            if cmd.valued.contains(&"--config") {
                assert!(cmd.usage.contains("[-c|--config PATH]"), "{}", cmd.usage);
            }
        }
        for sub in ["add", "list", "delete", "print", "update"] {
            assert!(
                docs.contains(&format!("`tuse plugins {sub}")),
                "docs misses tuse plugins {sub}"
            );
        }
    }
}
