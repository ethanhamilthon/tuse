use rusqlite::{Connection, OptionalExtension, params};
use std::sync::{Mutex, OnceLock};
use tuse_config::{ModelDef, Prices};

/// `~/.tuse/tuse.db`: sessions, their records, providers, models and the last choices.
/// One connection per process and per tuse home, opened on first use.
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
CREATE TABLE IF NOT EXISTS models (
    name TEXT PRIMARY KEY,
    display TEXT NOT NULL DEFAULT '',
    window INTEGER NOT NULL DEFAULT 0,
    compact_at INTEGER,
    price_in REAL,
    price_out REAL,
    price_cache REAL,
    price_cache_write REAL
);
CREATE TABLE IF NOT EXISTS state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Runs `f` on the database of the current `~/.tuse`; errors become strings.
pub fn with<R>(f: impl FnOnce(&Connection) -> rusqlite::Result<R>) -> Result<R, String> {
    let home = crate::store::tuse_home();
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

/// Providers saved by `tuse connect`.
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

fn model_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ModelDef> {
    let window: i64 = r.get(2)?;
    let compact_at: Option<i64> = r.get(3)?;
    let price_in: Option<f64> = r.get(4)?;
    let price_out: Option<f64> = r.get(5)?;
    let price_cache: Option<f64> = r.get(6)?;
    let price_cache_write: Option<f64> = r.get(7)?;
    let prices = if price_in.is_some()
        || price_out.is_some()
        || price_cache.is_some()
        || price_cache_write.is_some()
    {
        Some(Prices {
            input: price_in.unwrap_or(0.0),
            output: price_out.unwrap_or(0.0),
            cache_read: price_cache.unwrap_or(0.0),
            cache_write: price_cache_write.unwrap_or(0.0),
        })
    } else {
        None
    };
    Ok(ModelDef {
        name: r.get(0)?,
        display: r.get(1)?,
        window: window.max(0) as u64,
        compact_at: compact_at.map(|v| v.max(0) as u64),
        prices,
    })
}

/// Model metadata entries saved in SQLite (`tuse models`).
pub fn models() -> Vec<ModelDef> {
    with(|c| {
        let mut q = c.prepare(
            "SELECT name, display, window, compact_at, price_in, price_out, price_cache, price_cache_write
             FROM models ORDER BY name",
        )?;
        let rows = q.query_map([], model_row)?;
        rows.collect()
    })
    .unwrap_or_default()
}

pub fn model_def(name: &str) -> Option<ModelDef> {
    with(|c| {
        c.query_row(
            "SELECT name, display, window, compact_at, price_in, price_out, price_cache, price_cache_write
             FROM models WHERE name = ?1",
            [name],
            model_row,
        )
        .optional()
    })
    .ok()
    .flatten()
}

pub fn set_model_def(m: &ModelDef) -> Result<(), String> {
    if m.name.trim().is_empty() {
        return Err("model name must not be empty".into());
    }
    let (pi, po, pc, pw) = match &m.prices {
        Some(p) => (
            Some(p.input),
            Some(p.output),
            Some(p.cache_read),
            Some(p.cache_write),
        ),
        None => (None, None, None, None),
    };
    with(|c| {
        c.execute(
            "INSERT INTO models(name, display, window, compact_at, price_in, price_out, price_cache, price_cache_write)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(name) DO UPDATE SET
               display = ?2, window = ?3, compact_at = ?4,
               price_in = ?5, price_out = ?6, price_cache = ?7, price_cache_write = ?8",
            params![
                m.name,
                m.display,
                m.window as i64,
                m.compact_at.map(|v| v as i64),
                pi,
                po,
                pc,
                pw
            ],
        )
        .map(|_| ())
    })
}

pub fn remove_model_def(name: &str) -> Result<bool, String> {
    with(|c| {
        c.execute("DELETE FROM models WHERE name = ?1", [name])
            .map(|n| n > 0)
    })
}
