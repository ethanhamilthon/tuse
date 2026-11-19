use crate::args::Args;
use serde_json::Value;
use std::io::Read;
use tuse_socket::{Request, call};

fn session_id(a: &Args) -> Result<String, String> {
    a.value(&["-s", "--session"])
        .or_else(|| std::env::var("TUSE_SESSION").ok().filter(|s| !s.is_empty()))
        .ok_or_else(|| "no session: pass -s ID or set TUSE_SESSION".to_string())
}

fn send(a: &Args, req: Request) -> Result<Value, String> {
    let id = session_id(a)?;
    let resp = call(&tuse_core::socket_path(&id), &req)?;
    if resp.ok {
        Ok(resp.data)
    } else {
        Err(resp.error)
    }
}

fn text_arg(a: &Args) -> Result<String, String> {
    let joined = a.positional.join(" ");
    if joined == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        return Ok(s);
    }
    Ok(joined)
}

pub fn steer(a: &Args) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let files: Vec<String> = a
        .values(&["--file", "-f"])
        .into_iter()
        .map(|f| cwd.join(f).display().to_string())
        .collect();
    let text = text_arg(a)?;
    if text.trim().is_empty() && files.is_empty() {
        return Err("nothing to send".into());
    }
    send(a, Request::Steer { text, files }).map(|_| ())
}

pub fn ask(a: &Args) -> Result<(), String> {
    let mut pos = a.positional.clone();
    if pos.is_empty() {
        return Err("usage: tuse ask [--async] QUESTION OPTION...".into());
    }
    let question = pos.remove(0);
    let is_async = a.has(&["--async"]);
    let data = send(
        a,
        Request::Ask {
            question,
            options: pos,
            is_async,
        },
    )?;
    let key = if is_async { "id" } else { "answer" };
    println!("{}", data[key].as_str().unwrap_or(""));
    Ok(())
}

pub fn task(a: &Args) -> Result<(), String> {
    let (op, rest) = a
        .positional
        .split_first()
        .ok_or("usage: tuse task run|stop|get|list")?;
    let id = || rest.first().cloned().ok_or("task ID required".to_string());
    match op.as_str() {
        "run" => {
            if rest.is_empty() {
                return Err("usage: tuse task run [--name NAME] -- COMMAND...".into());
            }
            let cwd = std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string());
            let req = Request::TaskRun {
                command: rest.join(" "),
                name: a.value(&["--name"]),
                cwd,
            };
            println!("{}", send(a, req)?["id"].as_str().unwrap_or(""));
        }
        "stop" => {
            let t = send(a, Request::TaskStop { id: id()? })?;
            println!(
                "{} {}",
                t["id"].as_str().unwrap_or(""),
                t["status"].as_str().unwrap_or("")
            );
        }
        "get" => {
            let lines = a.value(&["--lines", "-n"]).and_then(|n| n.parse().ok());
            let d = send(a, Request::TaskGet { id: id()?, lines })?;
            println!(
                "{}\n{}",
                line(&d["task"]),
                d["output"].as_str().unwrap_or("")
            );
        }
        "list" => {
            let d = send(a, Request::TaskList)?;
            for t in d.as_array().into_iter().flatten() {
                println!("{}", line(t));
            }
        }
        other => return Err(format!("unknown task command `{other}`")),
    }
    Ok(())
}

fn line(t: &Value) -> String {
    let s = |k: &str| t[k].as_str().unwrap_or("").to_string();
    let status = match t["code"].as_i64() {
        Some(c) => format!("{} ({c})", s("status")),
        None => s("status"),
    };
    format!("{}\t{status}\t{}\t{}", s("id"), s("name"), s("log"))
}
