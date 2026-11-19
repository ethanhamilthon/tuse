use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub const REPL: &str = "repl";
pub const PROCESS: &str = "process";

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    /// `repl` (Python code in a background interpreter) or `process` (an `sh` that outlived its timeout).
    pub kind: String,
    /// The Python code a REPL task started with, or the command line of a process.
    pub code: String,
    pub cwd: String,
    /// `running`, `idle` (a REPL waiting for code), `exited` or `stopped`.
    pub status: String,
    /// Exit code of a process; `1` for a REPL that crashed.
    pub exit: Option<i32>,
    /// Snippets waiting in the queue of a REPL.
    pub queue: usize,
    pub started: String,
    pub ended: String,
    pub log: String,
    #[serde(skip)]
    pub pid: u32,
}

impl TaskInfo {
    /// Whether the task still holds a process or an interpreter.
    pub fn alive(&self) -> bool {
        matches!(self.status.as_str(), "running" | "idle")
    }
}

/// How to reach the thread of a live REPL task.
pub(crate) struct Live {
    pub tx: Sender<String>,
    pub cancel: Arc<AtomicBool>,
}

/// Background work of one session; output goes to `tasks/<id>.log`.
#[derive(Clone)]
pub struct Tasks {
    pub(crate) dir: PathBuf,
    pub(crate) list: Arc<Mutex<Vec<TaskInfo>>>,
    pub(crate) live: Arc<Mutex<HashMap<String, Live>>>,
}

/// Called when a task ends or a REPL goes idle: the task and the last line of its output.
pub type Notify = Arc<dyn Fn(TaskInfo, String) + Send + Sync>;

impl Tasks {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            list: Arc::default(),
            live: Arc::default(),
        }
    }

    /// A new entry and its log file.
    pub(crate) fn create(
        &self,
        kind: &str,
        name: Option<String>,
        code: &str,
        cwd: &Path,
        pid: u32,
    ) -> Result<(TaskInfo, File), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let mut list = self.list.lock().unwrap();
        let id = format!("t{}", list.len() + 1);
        let log = self.dir.join(format!("{id}.log"));
        let file = File::create(&log).map_err(|e| e.to_string())?;
        let info = TaskInfo {
            id,
            name: name.unwrap_or_else(|| first_line(code)),
            kind: kind.into(),
            code: code.to_string(),
            cwd: cwd.display().to_string(),
            status: "running".into(),
            exit: None,
            queue: 0,
            started: crate::id::now(),
            ended: String::new(),
            log: log.display().to_string(),
            pid,
        };
        list.push(info.clone());
        Ok((info, file))
    }

    pub(crate) fn update(&self, id: &str, f: impl FnOnce(&mut TaskInfo)) -> Option<TaskInfo> {
        let mut l = self.list.lock().unwrap();
        let t = l.iter_mut().find(|t| t.id == id)?;
        f(t);
        Some(t.clone())
    }

    /// Takes over a process whose `sh` timed out. `notify` runs when it exits on its own.
    pub fn adopt(
        &self,
        mut child: Child,
        cmd: &str,
        cwd: &Path,
        notify: Notify,
    ) -> Result<(String, File), Child> {
        let Ok((info, file)) = self.create(PROCESS, None, cmd, cwd, child.id()) else {
            return Err(child);
        };
        let (id, tasks) = (info.id.clone(), self.clone());
        std::thread::spawn(move || {
            let code = child.wait().ok().and_then(|s| s.code());
            let Some(done) = tasks.update(&id, |t| {
                t.ended = crate::id::now();
                t.exit = code;
                if t.status != "stopped" {
                    t.status = "exited".into();
                }
            }) else {
                return;
            };
            if done.status == "exited" {
                notify(done.clone(), tail(&done.log, 1));
            }
        });
        Ok((info.id, file))
    }

    /// Stops a task: kills a process, cancels a REPL and closes it.
    pub fn stop(&self, id: &str) -> Result<TaskInfo, String> {
        let t = self.get(id).ok_or(format!("no task {id}"))?;
        if !t.alive() {
            return Ok(t);
        }
        let done = self.update(id, |t| {
            t.status = "stopped".into();
            t.queue = 0;
        });
        match self.live.lock().unwrap().remove(id) {
            Some(live) => live
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed),
            None => tuse_py::kill_group(t.pid),
        }
        done.ok_or(format!("no task {id}"))
    }

    pub fn get(&self, id: &str) -> Option<TaskInfo> {
        self.list
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    pub fn list(&self) -> Vec<TaskInfo> {
        self.list.lock().unwrap().clone()
    }

    pub fn stop_all(&self) {
        for t in self.list() {
            if t.alive() {
                let _ = self.stop(&t.id);
            }
        }
    }
}

/// The last `lines` lines of a task log (reads at most the last 256 KiB).
pub fn tail(log: &str, lines: usize) -> String {
    let Ok(mut f) = File::open(log) else {
        return String::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(256 * 1024)));
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf);
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

fn first_line(s: &str) -> String {
    let l = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    l.trim().chars().take(60).collect()
}
