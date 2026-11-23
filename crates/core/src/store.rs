use crate::db;
use crate::record::Record;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};

pub fn tuse_home() -> PathBuf {
    tuse_config::tuse_home()
}

/// `~/.tuse/<id>/`: the lock, `tasks/` logs and pasted `images/`.
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
}

/// A session in the database. Holding it holds the session's lock. A new session
/// keeps its records in memory until its first message, so empty sessions leave nothing.
pub struct SessionDir {
    pub id: String,
    pub path: PathBuf,
    lock: Option<File>,
    pending: Vec<Record>,
    meta: Option<Meta>,
    seq: i64,
}

impl SessionDir {
    /// `create`: a new session, stored later; otherwise it is locked now.
    pub fn open(id: &str, create: bool) -> Result<Self, String> {
        let seq = db::with(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(seq), 0) FROM records WHERE session = ?1",
                [id],
                |r| r.get(0),
            )
        })?;
        let mut d = Self {
            id: id.to_string(),
            path: session_path(id),
            lock: None,
            pending: vec![],
            meta: None,
            seq,
        };
        if !create {
            d.materialize()?;
        }
        Ok(d)
    }

    fn materialize(&mut self) -> Result<(), String> {
        if self.lock.is_some() {
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
                    self.id
                ));
            }
            Err(TryLockError::Error(e)) => return Err(e.to_string()),
        }
        self.lock = Some(lock);
        for r in std::mem::take(&mut self.pending) {
            self.append(&r);
        }
        if let Some(m) = self.meta.take() {
            self.write_meta(&m);
        }
        Ok(())
    }

    pub fn append(&mut self, r: &Record) {
        if matches!(r, Record::User { .. }) {
            let _ = self.materialize();
        }
        if self.lock.is_none() {
            self.pending.push(r.clone());
            return;
        }
        let Ok(body) = serde_json::to_string(r) else {
            return;
        };
        self.seq += 1;
        let (id, seq) = (&self.id, self.seq);
        let _ = db::with(|c| {
            c.execute(
                "INSERT INTO records(session, seq, body) VALUES (?1, ?2, ?3)",
                params![id, seq, body],
            )
        });
    }

    pub fn records(&self) -> Vec<Record> {
        read_records(&self.id)
    }

    pub fn write_meta(&mut self, m: &Meta) {
        if self.lock.is_none() {
            self.meta = Some(m.clone());
            return;
        }
        let _ = put_meta(m);
    }
}

fn put_meta(m: &Meta) -> Result<(), String> {
    let body = serde_json::to_string(m).map_err(|e| e.to_string())?;
    db::with(|c| {
        c.execute(
            "INSERT INTO sessions(id, meta, cwd, updated) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET meta = ?2, cwd = ?3, updated = ?4",
            params![m.id, body, m.cwd, m.updated],
        )
        .map(|_| ())
    })
}

pub fn read_records(id: &str) -> Vec<Record> {
    db::with(|c| {
        let mut q = c.prepare("SELECT body FROM records WHERE session = ?1 ORDER BY seq")?;
        let rows = q.query_map([id], |r| r.get::<_, String>(0))?;
        Ok(rows
            .flatten()
            .filter_map(|b| serde_json::from_str(&b).ok())
            .collect())
    })
    .unwrap_or_default()
}

/// Fails if `id` is currently locked by another running process.
pub(crate) fn check_unlocked(id: &str) -> Result<(), String> {
    let lock = session_path(id).join("lock");
    if !lock.exists() {
        return Ok(());
    }
    let file = File::open(&lock).map_err(|e| e.to_string())?;
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => {
            Err(format!("session {id} is open in another tuse process"))
        }
        Err(TryLockError::Error(e)) => Err(e.to_string()),
    }
}

pub fn read_meta(id: &str) -> Option<Meta> {
    let body: Option<String> = db::with(|c| {
        c.query_row("SELECT meta FROM sessions WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()
    })
    .ok()
    .flatten();
    serde_json::from_str(&body?).ok()
}

fn metas(sql: &str, arg: &str) -> Vec<Meta> {
    db::with(|c| {
        let mut q = c.prepare(sql)?;
        let rows = q.query_map([arg], |r| r.get::<_, String>(0))?;
        Ok(rows
            .flatten()
            .filter_map(|b| serde_json::from_str(&b).ok())
            .collect())
    })
    .unwrap_or_default()
}

/// Sessions started in `cwd` that have at least one message, newest first.
pub fn list_sessions(cwd: &Path) -> Vec<Meta> {
    let cwd = cwd.display().to_string();
    metas(
        "SELECT meta FROM sessions WHERE cwd = ?1 ORDER BY updated DESC",
        &cwd,
    )
    .into_iter()
    .filter(|m: &Meta| !m.title.is_empty())
    .collect()
}

/// The last model and effort picked in the UI win over the config file.
pub(crate) fn apply_state(config: &mut tuse_config::Config) {
    config.connect = db::providers();
    for c in &mut config.connect {
        c.api_key = tuse_config::env_value(&c.api_key);
    }
    for (key, slot) in [("model", &mut config.model), ("effort", &mut config.effort)] {
        if let Some(v) = db::get_state(key).filter(|v| !v.is_empty()) {
            *slot = v;
        }
    }
}
