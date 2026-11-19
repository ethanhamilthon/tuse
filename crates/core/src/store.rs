use crate::record::Record;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub fn tuse_home() -> PathBuf {
    match std::env::var_os("TUSE_HOME") {
        Some(p) => PathBuf::from(p),
        None => tuse_config::home().join(".tuse"),
    }
}

pub fn session_path(id: &str) -> PathBuf {
    tuse_home().join(id)
}

/// `~/.tuse/<id>/sock`, or `/tmp/tuse-$USER/<id>.sock` when that path is too
/// long for a Unix socket (about 104 bytes).
pub fn socket_path(id: &str) -> PathBuf {
    let p = session_path(id).join("sock");
    if p.as_os_str().len() < 100 {
        return p;
    }
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    let dir = PathBuf::from(format!("/tmp/tuse-{user}"));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{}.sock", &id[id.len().saturating_sub(16)..]))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    pub cwd: String,
    pub title: String,
    pub model: String,
    pub effort: String,
    pub created: String,
    pub updated: String,
}

/// `~/.tuse/<id>/`: `session.jsonl`, `meta.json`, `lock`, `sock`, `bin/`, `tasks/`.
/// Holding a `SessionDir` holds the session's lock.
pub struct SessionDir {
    pub path: PathBuf,
    _lock: File,
    log: File,
}

impl SessionDir {
    pub fn open(id: &str) -> Result<Self, String> {
        let path = session_path(id);
        std::fs::create_dir_all(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let lock = File::create(path.join("lock")).map_err(|e| e.to_string())?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(format!("session {id} is open in another tuse process"));
            }
            Err(TryLockError::Error(e)) => return Err(e.to_string()),
        }
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.join("session.jsonl"))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            _lock: lock,
            log,
        })
    }

    pub fn append(&mut self, r: &Record) {
        if let Ok(mut line) = serde_json::to_string(r) {
            line.push('\n');
            let _ = self.log.write_all(line.as_bytes());
        }
    }

    pub fn records(&self) -> Vec<Record> {
        let Ok(f) = File::open(self.path.join("session.jsonl")) else {
            return vec![];
        };
        BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter_map(|l| serde_json::from_str(&l).ok())
            .collect()
    }

    pub fn write_meta(&self, m: &Meta) {
        if let Ok(text) = serde_json::to_string_pretty(m) {
            let _ = std::fs::write(self.path.join("meta.json"), text);
        }
    }
}

pub fn read_meta(dir: &Path) -> Option<Meta> {
    serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json")).ok()?).ok()
}

/// Sessions started in `cwd` that have at least one message, newest first.
pub fn list_sessions(cwd: &Path) -> Vec<Meta> {
    let cwd = cwd.display().to_string();
    let mut out: Vec<Meta> = std::fs::read_dir(tuse_home())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| read_meta(&e.path()))
        .filter(|m| m.cwd == cwd && !m.title.is_empty())
        .collect();
    out.sort_by(|a, b| b.updated.cmp(&a.updated));
    out
}
