use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub command: String,
    pub cwd: String,
    pub status: String,
    pub code: Option<i32>,
    pub started: String,
    pub ended: String,
    pub log: String,
    #[serde(skip)]
    pub pid: u32,
}

impl TaskInfo {
    pub fn running(&self) -> bool {
        self.status == "running"
    }
}

/// Background processes of one session; output goes to `tasks/<id>.log`.
#[derive(Clone)]
pub struct Tasks {
    dir: PathBuf,
    list: Arc<Mutex<Vec<TaskInfo>>>,
}

impl Tasks {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            list: Arc::default(),
        }
    }

    /// Starts `bash -c command`; `on_exit` runs when it exits on its own.
    pub fn run(
        &self,
        command: &str,
        name: Option<String>,
        cwd: &Path,
        env: &[(String, String)],
        on_exit: Box<dyn FnOnce(TaskInfo) + Send>,
    ) -> Result<TaskInfo, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let id = format!("t{}", self.list.lock().unwrap().len() + 1);
        let log = self.dir.join(format!("{id}.log"));
        let out = File::create(&log).map_err(|e| e.to_string())?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let mut c = Command::new("bash");
        c.arg("-c")
            .arg(command)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .process_group(0);
        for (k, v) in env {
            c.env(k, v);
        }
        let mut child = c.spawn().map_err(|e| e.to_string())?;
        let info = TaskInfo {
            id: id.clone(),
            name: name.unwrap_or_else(|| first_line(command)),
            command: command.to_string(),
            cwd: cwd.display().to_string(),
            status: "running".into(),
            code: None,
            started: crate::id::now(),
            ended: String::new(),
            log: log.display().to_string(),
            pid: child.id(),
        };
        self.list.lock().unwrap().push(info.clone());
        let list = self.list.clone();
        std::thread::spawn(move || {
            let code = child.wait().ok().and_then(|s| s.code());
            let mut l = list.lock().unwrap();
            let Some(t) = l.iter_mut().find(|t| t.id == id) else {
                return;
            };
            t.ended = crate::id::now();
            t.code = code;
            let stopped = t.status == "stopped";
            if !stopped {
                t.status = "exited".into();
            }
            let done = t.clone();
            drop(l);
            if !stopped {
                on_exit(done);
            }
        });
        Ok(info)
    }

    pub fn stop(&self, id: &str) -> Result<TaskInfo, String> {
        let mut l = self.list.lock().unwrap();
        let t = l
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(format!("no task {id}"))?;
        if t.running() {
            t.status = "stopped".into();
            crate::bash::kill_group(t.pid);
        }
        Ok(t.clone())
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
            if t.running() {
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
    let l = s.lines().next().unwrap_or("");
    l.chars().take(60).collect()
}
