mod args;
mod connect;
mod headless;
mod update;

use args::Args;
use std::io::Read;

use tuse_core::{Options, Session};

const USAGE: &str = "tuse - minimal AI coding agent

USAGE
  tuse [-c|--config PATH] [-r|--resume ID] open the TUI; -c picks the config file,
                                           -r ID opens a saved session (its ID is printed
                                           to stderr on exit, see /resume)
  tuse -p [PROMPT|-] [-c PATH] [-r ID]
                                           headless (`-` reads the prompt from stdin):
                                           runs the prompt to the end, replies to stdout,
                                           the session ID to stderr; exit code 1 on an error
  tuse json [-c PATH] [-r ID]              a JSON-RPC 2.0 session over stdin/stdout, one
                                           message per line (see docs/json.md)
  tuse connect [--config PATH]            add a provider: asks base url, api key and name,
                                           fetches its models and saves it to the config
  tuse reset [--yes]                      replace the global config folder with the latest
                                           main.yaml
  tuse update                             update the binary
  tuse version | tuse help

The config is --config, else ./tuse.yaml, else $TUSE_GLOBAL_CONFIG, else
~/.config/tuse/main.yaml.";

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
        "connect" => {
            let a = Args::parse(rest, &["--config", "-c"])?;
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            connect::run(a.value(&["--config", "-c"]).map(|p| cwd.join(p)))
        }
        "json" => json(&Args::parse(rest, &["--config", "-c", "--resume", "-r"])?),
        "reset" => update::reset(Args::parse(rest, &[])?.has(&["--yes", "-y"])),
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
    if let Some(f) = a.unknown(&["--config", "-c", "--resume", "-r", "-p", "--print"]) {
        return Err(format!("unknown flag {f}\n\n{USAGE}"));
    }
    let headless = a.has(&["-p", "--print"]);
    if !headless && !a.positional.is_empty() {
        return Err(format!("unknown command `{}`\n\n{USAGE}", a.positional[0]));
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let config = a.value(&["--config", "-c"]).map(|p| cwd.join(p));
    let session = Session::open(Options {
        cwd: cwd.clone(),
        config: config.clone(),
        mode: if headless {
            tuse_core::Mode::Headless
        } else {
            tuse_core::Mode::Tui
        },
        resume: a.value(&["--resume", "-r"]),
        start: tuse_core::Start::Launch,
    })?;
    if headless {
        let mut prompt = a.positional.join(" ");
        if prompt == "-" {
            prompt.clear();
            std::io::stdin()
                .read_to_string(&mut prompt)
                .map_err(|e| e.to_string())?;
        }
        return headless::run(session, Some(prompt));
    }
    let id = tuse_tui::run(session, cwd, config).map_err(|e| e.to_string())?;
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
