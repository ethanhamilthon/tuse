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
    api_key TEXT NOT NULL,
    api TEXT NOT NULL DEFAULT 'openai-chat'
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
            .and_then(|_| migrate(&c))
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

fn migrate(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch("BEGIN IMMEDIATE")?;
    let res = (|| {
        let mut q = c.prepare("PRAGMA table_info(providers)")?;
        let cols: Vec<String> = q
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<_, _>>()?;
        if !cols.iter().any(|col| col == "api") {
            c.execute(
                "ALTER TABLE providers ADD COLUMN api TEXT NOT NULL DEFAULT 'openai-chat'",
                [],
            )?;
        }
        Ok(())
    })();
    match res {
        Ok(()) => c.execute_batch("COMMIT"),
        Err(e) => {
            let _ = c.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

pub fn providers() -> Result<Vec<tuse_config::Connect>, String> {
    with(|c| {
        let mut q =
            c.prepare("SELECT name, base_url, api_key, api FROM providers ORDER BY rowid")?;
        let rows = q.query_map([], |r| {
            let mut conn = tuse_config::Connect {
                name: r.get(0)?,
                base_url: r.get(1)?,
                api_key: r.get(2)?,
                api: r.get(3)?,
            };
            conn.api = conn.api_type().to_string();
            Ok(conn)
        })?;
        rows.collect()
    })
}

pub fn add_provider(p: &tuse_config::Connect) -> Result<(), String> {
    let api = p.api_type();
    with(|c| {
        c.execute(
            "INSERT INTO providers(name, base_url, api_key, api) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET base_url = ?2, api_key = ?3, api = ?4",
            params![p.name, p.base_url, p.api_key, api],
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

pub fn providers_json() -> Result<String, String> {
    let rows: Vec<_> = providers()?
        .into_iter()
        .map(|p| {
            let api = p.api_type().to_string();
            serde_json::json!({
                "name": p.name,
                "base_url": p.base_url,
                "api": api,
            })
        })
        .collect();
    serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_migrations_upgrade_legacy_providers_safely() {
        let dir = std::env::temp_dir().join(format!("tuse-db-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tuse.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE providers (name TEXT PRIMARY KEY, base_url TEXT NOT NULL, api_key TEXT NOT NULL);
                 INSERT INTO providers VALUES ('old', 'http://127.0.0.1:1', 'k');",
            )
            .unwrap();
        }
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let p = path.clone();
                let b = barrier.clone();
                std::thread::spawn(move || {
                    let c = Connection::open(&p).unwrap();
                    c.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
                    b.wait();
                    migrate(&c).unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let c = Connection::open(&path).unwrap();
        let api: String = c
            .query_row("SELECT api FROM providers WHERE name = 'old'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(api, "openai-chat");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
