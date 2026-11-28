use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub started: String,
    pub ended: String,
    pub log: String,
    pub code: String,
}

impl TaskInfo {
    pub fn alive(&self) -> bool {
        self.status == "running"
    }
}

#[derive(Clone)]
pub struct Tasks {
    dir: PathBuf,
    list: Arc<Mutex<Vec<TaskInfo>>>,
    running: Arc<Mutex<HashMap<String, RunningTask>>>,
}

struct RunningTask {
    cancel: Arc<AtomicBool>,
    started: Instant,
}

impl Tasks {
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

    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            list: Arc::default(),
            running: Arc::default(),
        }
    }

    pub(crate) fn create(&self, name: &str, code: &str) -> Result<tuse_lua::TaskStart, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let mut list = self.list.lock().unwrap();
        let mut next = list
            .last()
            .and_then(|t| t.id.strip_prefix('t')?.parse::<usize>().ok())
            .unwrap_or(0);
        let (id, log, file) = loop {
            next += 1;
            let id = format!("t{next}");
            let log = self.dir.join(format!("{id}.log"));
            match File::create_new(&log) {
                Ok(file) => break (id, log, file),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        };
        list.push(TaskInfo {
            id: id.clone(),
            name: if name.is_empty() {
                id.clone()
            } else {
                name.to_string()
            },
            status: "running".into(),
            started: crate::id::now(),
            ended: String::new(),
            log: log.display().to_string(),
            code: code.to_string(),
        });
        let cancel = Arc::new(AtomicBool::new(false));
        self.running.lock().unwrap().insert(
            id.clone(),
            RunningTask {
                cancel: cancel.clone(),
                started: Instant::now(),
            },
        );
        Ok(tuse_lua::TaskStart {
            id,
            log: file,
            cancel,
        })
    }

    pub(crate) fn end(&self, id: &str, ok: bool) -> Option<(TaskInfo, Duration)> {
        let mut l = self.list.lock().unwrap();
        let run = self.running.lock().unwrap().remove(id)?;
        let t = l.iter_mut().find(|t| t.id == id)?;
        t.ended = crate::id::now();
        if t.status != "stopped" {
            t.status = if ok { "done" } else { "failed" }.into();
        }
        Some((t.clone(), run.started.elapsed()))
    }

    pub fn stop(&self, id: &str) -> Result<(TaskInfo, Option<Duration>), String> {
        let mut l = self.list.lock().unwrap();
        let t = l
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(format!("no task {id}"))?;
        let duration = if t.alive() {
            t.status = "stopped".into();
            t.ended = crate::id::now();
            self.running.lock().unwrap().remove(id).map(|c| {
                c.cancel.store(true, Ordering::Relaxed);
                c.started.elapsed()
            })
        } else {
            None
        };
        Ok((t.clone(), duration))
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

    pub fn running(&self) -> usize {
        self.list
            .lock()
            .unwrap()
            .iter()
            .filter(|t| t.alive())
            .count()
    }
}
