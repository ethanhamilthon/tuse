mod args;
mod connect;
mod init;
mod models;
mod update;

use args::Args;

use tuse_core::{Options, Session};

const USAGE: &str = "tuse - minimal AI coding agent

USAGE
  tuse [-c|--config PATH] [-r|--resume ID] open the TUI; -c picks the config file,
                                           -r ID opens a saved session (its ID is printed
                                           to stderr on exit, see /resume)
  tuse json [-c PATH] [-r ID]              a JSON-RPC 2.0 session over stdin/stdout, one
                                           message per line (see docs/json.md)
  tuse connect --base URL --key '$ENV' --name NAME
                                           verify and save an API provider
  tuse disconnect NAME                    remove a provider
  tuse providers                          list the providers
  tuse models                             list configured model metadata and the saved default
  tuse init                               create ./tuse.lua that extends the global config
  tuse update                             update the binary
  tuse version | tuse help

The config is --config, else ./tuse.lua, else ~/.config/tuse.lua or
~/.config/tuse/init.lua: a Lua file that returns a table.";

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(raw) {
        eprintln!("tuse: {e}");
        std::process::exit(1);
    }
}

fn run(raw: Vec<String>) -> Result<(), String> {
    let sub = raw.first().map(String::as_str).unwrap_or("");
    let rest = raw.get(1..).unwrap_or_default();
    match sub {
        "connect" => connect::run(rest),
        "disconnect" => match rest {
            [name] => connect::remove(name),
            _ => Err(format!("usage: tuse disconnect NAME\n\n{USAGE}")),
        },
        "providers" => {
            connect::list();
            Ok(())
        }
        "models" => models::run(rest),
        "json" => json(&Args::parse(rest, &["--config", "-c", "--resume", "-r"])?),
        "init" => init::run(rest),
        "update" => update::update(),
        "version" | "--version" | "-V" => {
            println!("tuse {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        _ => start(&Args::parse(&raw, &["--config", "-c", "--resume", "-r"])?),
    }
}

fn start(a: &Args) -> Result<(), String> {
    if let Some(f) = a.unknown(&["--config", "-c", "--resume", "-r"]) {
        return Err(format!("unknown flag {f}\n\n{USAGE}"));
    }
    if let Some(p) = a.positional.first() {
        return Err(format!("unknown command `{p}`\n\n{USAGE}"));
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let config = a.value(&["--config", "-c"]).map(|p| cwd.join(p));
    let session = Session::open(Options {
        cwd,
        config: config.clone(),
        mode: tuse_core::Mode::Tui,
        resume: a.value(&["--resume", "-r"]),
        start: tuse_core::Start::Launch,
    })?;
    let id = tuse_tui::run(session, config).map_err(|e| e.to_string())?;
    eprintln!("tuse: session {id}");
    Ok(())
}

/// `tuse json`: stdout carries only the protocol; errors go to stderr.
fn json(a: &Args) -> Result<(), String> {
    if let Some(f) = a.unknown(&["--config", "-c", "--resume", "-r"]) {
        return Err(format!("unknown flag {f}\n\n{USAGE}"));
    }
    if let Some(p) = a.positional.first() {
        return Err(format!("unexpected argument `{p}`\n\n{USAGE}"));
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let session = Session::open(Options {
        cwd: cwd.clone(),
        config: a.value(&["--config", "-c"]).map(|p| cwd.join(p)),
        mode: tuse_core::Mode::Json,
        resume: a.value(&["--resume", "-r"]),
        start: tuse_core::Start::Launch,
    })?;
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
