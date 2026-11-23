use rusqlite::{Connection, OptionalExtension, params};
use std::sync::{Mutex, OnceLock};

static DB: OnceLock<Mutex<Vec<(std::path::PathBuf, Connection)>>> = OnceLock::new();

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    meta TEXT NOT NULL,
    cwd TEXT NOT NULL,
    updated TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS sessions_cwd ON sessions(cwd);
CREATE TABLE IF NOT EXISTS records (
    session TEXT NOT NULL,
    seq INTEGER NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY (session, seq)
);
CREATE TABLE IF NOT EXISTS providers (
    name TEXT PRIMARY KEY,
    base_url TEXT NOT NULL,
    api_key TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

pub fn with<R>(f: impl FnOnce(&Connection) -> rusqlite::Result<R>) -> Result<R, String> {
    let home = tuse_config::tuse_home();
    let mut all = DB.get_or_init(Mutex::default).lock().unwrap();
    if !all.iter().any(|(h, _)| *h == home) {
        std::fs::create_dir_all(&home).map_err(|e| format!("{}: {e}", home.display()))?;
        let c = Connection::open(home.join("tuse.db")).map_err(|e| e.to_string())?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .and_then(|_| c.pragma_update(None, "journal_mode", "WAL"))
            .and_then(|_| c.execute_batch(SCHEMA))
            .map_err(|e| e.to_string())?;
        all.push((home.clone(), c));
    }
    let c = &all.iter().find(|(h, _)| *h == home).unwrap().1;
    f(c).map_err(|e| e.to_string())
}

pub fn get_state(key: &str) -> Option<String> {
    with(|c| {
        c.query_row("SELECT value FROM state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()
    })
    .ok()
    .flatten()
}

pub fn set_state(key: &str, value: &str) -> Result<(), String> {
    with(|c| {
        c.execute(
            "INSERT INTO state(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )
        .map(|_| ())
    })
}

pub fn default_model() -> String {
    get_state("model").unwrap_or_default()
}

pub fn default_effort() -> String {
    match get_state("effort") {
        Some(e) if tuse_config::EFFORTS.contains(&e.as_str()) => e,
        _ => "default".into(),
    }
}

pub fn providers() -> Vec<tuse_config::Connect> {
    with(|c| {
        let mut q = c.prepare("SELECT name, base_url, api_key FROM providers ORDER BY rowid")?;
        let rows = q.query_map([], |r| {
            Ok(tuse_config::Connect {
                name: r.get(0)?,
                base_url: r.get(1)?,
                api_key: r.get(2)?,
            })
        })?;
        rows.collect()
    })
    .unwrap_or_default()
}

pub fn add_provider(p: &tuse_config::Connect) -> Result<(), String> {
    with(|c| {
        c.execute(
            "INSERT INTO providers(name, base_url, api_key) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET base_url = ?2, api_key = ?3",
            params![p.name, p.base_url, p.api_key],
        )
        .map(|_| ())
    })
}

pub fn remove_provider(name: &str) -> Result<bool, String> {
    with(|c| {
        c.execute("DELETE FROM providers WHERE name = ?1", [name])
            .map(|n| n > 0)
    })
}
