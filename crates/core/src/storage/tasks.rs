use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub started: String,
    pub ended: String,
    pub log: String,
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
    cancels: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
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
            cancels: Arc::default(),
        }
    }

    pub(crate) fn create(&self, name: &str) -> Result<tuse_lua::TaskStart, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let mut list = self.list.lock().unwrap();
        let id = format!("t{}", list.len() + 1);
        let log = self.dir.join(format!("{id}.log"));
        File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
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
        });
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels
            .lock()
            .unwrap()
            .insert(id.clone(), cancel.clone());
        Ok(tuse_lua::TaskStart { id, log, cancel })
    }

    pub(crate) fn end(&self, id: &str, ok: bool) -> Option<TaskInfo> {
        self.cancels.lock().unwrap().remove(id);
        let mut l = self.list.lock().unwrap();
        let t = l.iter_mut().find(|t| t.id == id)?;
        t.ended = crate::id::now();
        if t.status == "stopped" {
            return None;
        }
        t.status = if ok { "done" } else { "failed" }.into();
        Some(t.clone())
    }

    pub fn stop(&self, id: &str) -> Result<TaskInfo, String> {
        let mut l = self.list.lock().unwrap();
        let t = l
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(format!("no task {id}"))?;
        if t.alive() {
            t.status = "stopped".into();
            if let Some(c) = self.cancels.lock().unwrap().get(id) {
                c.store(true, Ordering::Relaxed);
            }
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
            if t.alive() {
                let _ = self.stop(&t.id);
            }
        }
    }
}
