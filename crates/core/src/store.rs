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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    pub cwd: String,
    pub title: String,
    pub model: String,
    pub effort: String,
    pub created: String,
    pub updated: String,
    /// When the process holding the session ended; empty for older sessions.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub closed: String,
    /// The session whose process started this one (`TUSE_PARENT`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parent: String,
}

/// `~/.tuse/<id>/`: `session.jsonl`, `meta.json`, `lock`, `tasks/` and pasted `images/`.
/// Holding a `SessionDir` holds the session's lock. A new session keeps its records
/// in memory and creates the folder with its first message, so empty sessions leave nothing.
pub struct SessionDir {
    pub path: PathBuf,
    files: Option<(File, File)>,
    pending: Vec<String>,
    meta: Option<Meta>,
}

impl SessionDir {
    /// `create`: a new session, whose folder appears later; otherwise the folder is opened now.
    pub fn open(id: &str, create: bool) -> Result<Self, String> {
        let mut d = Self {
            path: session_path(id),
            files: None,
            pending: vec![],
            meta: None,
        };
        if !create {
            d.materialize()?;
        }
        Ok(d)
    }

    pub(crate) fn materialize(&mut self) -> Result<(), String> {
        if self.files.is_some() {
            return Ok(());
        }
        let path = &self.path;
        std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let lock = File::create(path.join("lock")).map_err(|e| e.to_string())?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(format!(
                    "session {} is open in another tuse process",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Err(TryLockError::Error(e)) => return Err(e.to_string()),
        }
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.join("session.jsonl"))
            .map_err(|e| e.to_string())?;
        for line in self.pending.drain(..) {
            let _ = log.write_all(line.as_bytes());
        }
        self.files = Some((lock, log));
        if let Some(m) = self.meta.take() {
            self.write_meta(&m);
        }
        Ok(())
    }

    pub fn append(&mut self, r: &Record) {
        let Ok(mut line) = serde_json::to_string(r) else {
            return;
        };
        line.push('\n');
        if matches!(r, Record::User { .. }) {
            let _ = self.materialize();
        }
        match &mut self.files {
            Some((_, log)) => {
                let _ = log.write_all(line.as_bytes());
            }
            None => self.pending.push(line),
        }
    }

    pub fn records(&self) -> Vec<Record> {
        read_records(&self.path)
    }

    pub fn write_meta(&mut self, m: &Meta) {
        if self.files.is_none() {
            self.meta = Some(m.clone());
            return;
        }
        if let Ok(text) = serde_json::to_string_pretty(m) {
            let _ = std::fs::write(self.path.join("meta.json"), text);
        }
    }
}

/// Lines that do not parse (a half-written last one) are skipped.
pub fn read_records(dir: &Path) -> Vec<Record> {
    let Ok(f) = File::open(dir.join("session.jsonl")) else {
        return vec![];
    };
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}

/// Whether some process holds the session's lock. The probe takes a shared lock and drops it at once.
pub fn is_locked(dir: &Path) -> bool {
    File::open(dir.join("lock"))
        .is_ok_and(|f| matches!(f.try_lock_shared(), Err(TryLockError::WouldBlock)))
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
