mod args;
mod connect;
mod headless;
mod update;

use args::Args;
use std::io::Read;

use tuse_core::{Options, Session};

const USAGE: &str = "tuse - minimal AI coding agent

USAGE
  tuse [--config PATH] [--resume ID]       open the TUI
  tuse -p [PROMPT|-] [--config PATH] [--resume ID]
                                           headless (`-` reads the prompt from stdin):
                                           runs the prompt to the end, replies to stdout,
                                           the session ID to stderr; exit code 1 on an error
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
        headless,
        resume: a.value(&["--resume", "-r"]),
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
