mod args;
mod connect;
mod headless;
mod remote;

use args::Args;
use std::io::Read;

use tuse_core::{Options, Session};

const USAGE: &str = "tuse - minimal AI coding agent

USAGE
  tuse [--config PATH] [--resume ID]       open the TUI
  tuse -p [PROMPT|-] [--config PATH] [--resume ID]
                                           headless (`-` reads the prompt from stdin):
                                           session ID to stderr, replies to stdout;
                                           runs until killed, takes `tuse steer`
  tuse steer [-s ID] [--file PATH]... [TEXT|-]
                                           send a message to a running session
  tuse ask [-s ID] [--async] QUESTION OPTION...
                                           ask the TUI user; prints the answer
                                           (--async: prints an ID, the answer is steered back)
  tuse task run [-s ID] [--name NAME] -- COMMAND...
  tuse task stop|get [--lines N]|list [-s ID] [ID]
                                           background tasks of a session
  tuse connect [--config PATH]            add a provider: asks base url, api key and name,
                                           fetches its models and saves it to the config
  tuse version | tuse help

-s defaults to $TUSE_SESSION. The config is --config, else ./tuse.yaml,
else ~/.config/tuse/main.yaml.";

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
    let session_flags = ["-s", "--session"];
    match sub {
        "steer" => remote::steer(&Args::parse(rest, &["-s", "--session", "--file", "-f"])?),
        "ask" => remote::ask(&Args::parse(rest, &session_flags)?),
        "task" => remote::task(&Args::parse(
            rest,
            &["-s", "--session", "--name", "--lines", "-n"],
        )?),
        "connect" => {
            let a = Args::parse(rest, &["--config", "-c"])?;
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            connect::run(a.value(&["--config", "-c"]).map(|p| cwd.join(p)))
        }
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
