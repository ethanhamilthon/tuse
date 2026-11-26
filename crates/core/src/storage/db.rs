use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::sync::{Mutex, OnceLock};

static DB: OnceLock<Mutex<Vec<(std::path::PathBuf, Connection)>>> = OnceLock::new();

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    cwd TEXT NOT NULL,
    title TEXT NOT NULL,
    model TEXT NOT NULL,
    effort TEXT NOT NULL,
    started INTEGER NOT NULL DEFAULT 0,
    usage_input INTEGER NOT NULL DEFAULT 0,
    usage_output INTEGER NOT NULL DEFAULT 0,
    usage_cached INTEGER NOT NULL DEFAULT 0,
    usage_cache_write INTEGER NOT NULL DEFAULT 0,
    cost REAL NOT NULL DEFAULT 0,
    llm_cost REAL NOT NULL DEFAULT 0,
    priced INTEGER NOT NULL DEFAULT 0,
    ctx INTEGER NOT NULL DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 0,
    created TEXT NOT NULL,
    updated TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_cwd ON sessions(cwd);
CREATE TABLE IF NOT EXISTS history (
    session TEXT NOT NULL,
    seq INTEGER NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY (session, seq)
);
CREATE TABLE IF NOT EXISTS providers (
    name TEXT PRIMARY KEY,
    base_url TEXT NOT NULL,
    api_key TEXT NOT NULL,
    api TEXT NOT NULL DEFAULT 'openai-chat',
    headers TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

pub(super) fn with<R>(f: impl FnOnce(&Connection) -> rusqlite::Result<R>) -> Result<R, String> {
    let home = tuse_config::tuse_home();
    let mut all = DB.get_or_init(Mutex::default).lock().unwrap();
    if !all.iter().any(|(h, _)| *h == home) {
        std::fs::create_dir_all(&home).map_err(|e| format!("{}: {e}", home.display()))?;
        let mut c = Connection::open(home.join("tuse.db")).map_err(|e| e.to_string())?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .and_then(|_| c.pragma_update(None, "journal_mode", "WAL"))
            .and_then(|_| initialize_schema(&mut c))
            .map_err(|e| e.to_string())?;
        all.push((home.clone(), c));
    }
    let c = &all.iter().find(|(h, _)| *h == home).unwrap().1;
    f(c).map_err(|e| e.to_string())
}

fn initialize_schema(c: &mut Connection) -> rusqlite::Result<()> {
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 {
        tx.execute_batch(
            "DROP TABLE IF EXISTS sessions; DROP TABLE IF EXISTS records;
            DROP TABLE IF EXISTS providers; DROP TABLE IF EXISTS state;",
        )?;
        tx.execute_batch(SCHEMA)?;
    } else if version < 3 {
        if version == 1 {
            tx.execute_batch(
                "ALTER TABLE providers ADD COLUMN headers TEXT NOT NULL DEFAULT '{}';",
            )?;
        }
        tx.execute_batch("DROP TABLE IF EXISTS sessions; DROP TABLE IF EXISTS history;")?;
        tx.execute_batch(SCHEMA)?;
    }
    if version < 3 {
        tx.pragma_update(None, "user_version", 3)?;
    }

    tx.commit()
}

pub(super) fn transaction<R>(
    f: impl FnOnce(&rusqlite::Transaction<'_>) -> rusqlite::Result<R>,
) -> Result<R, String> {
    with(|c| {
        let tx = rusqlite::Transaction::new_unchecked(c, TransactionBehavior::Immediate)?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    })
}

pub struct Prefs;

impl Prefs {
    pub fn set(key: &str, value: &str) -> Result<(), String> {
        with(|c| {
            c.execute(
                "INSERT INTO state(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
                params![key, value],
            )
            .map(|_| ())
        })
    }

    pub fn model() -> String {
        get_state("model").unwrap_or_default()
    }

    pub fn effort() -> String {
        match get_state("effort") {
            Some(e) if tuse_config::EFFORTS.contains(&e.as_str()) => e,
            _ => "default".into(),
        }
    }
}

fn get_state(key: &str) -> Option<String> {
    with(|c| {
        c.query_row("SELECT value FROM state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()
    })
    .ok()
    .flatten()
}

pub struct Providers;

impl Providers {
    pub fn list() -> Result<Vec<tuse_config::Connect>, String> {
        with(|c| {
            let mut q = c.prepare(
                "SELECT name, base_url, api_key, api, headers FROM providers ORDER BY rowid",
            )?;
            let rows = q.query_map([], |r| {
                let mut conn = tuse_config::Connect {
                    name: r.get(0)?,
                    base_url: r.get(1)?,
                    api_key: r.get(2)?,
                    api: r.get(3)?,
                    headers: serde_json::from_str(&r.get::<_, String>(4)?).map_err(|_| {
                        rusqlite::Error::InvalidColumnType(
                            4,
                            "headers".into(),
                            rusqlite::types::Type::Text,
                        )
                    })?,
                };
                conn.api = conn.api_type().to_string();
                Ok(conn)
            })?;
            rows.collect()
        })
    }

    pub fn add(p: &tuse_config::Connect) -> Result<(), String> {
        let api = p.api_type();
        let headers = serde_json::to_string(&p.headers).map_err(|e| e.to_string())?;
        with(|c| {
            c.execute(
                "INSERT INTO providers(name, base_url, api_key, api, headers) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(name) DO UPDATE SET base_url = ?2, api_key = ?3, api = ?4, headers = ?5",
                params![p.name, p.base_url, p.api_key, api, headers],
            )
            .map(|_| ())
        })
    }

    pub fn remove(name: &str) -> Result<bool, String> {
        with(|c| {
            c.execute("DELETE FROM providers WHERE name = ?1", [name])
                .map(|n| n > 0)
        })
    }

    pub fn json() -> Result<String, String> {
        let rows: Vec<_> = Providers::list()?
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

    pub(crate) fn apply(config: &mut tuse_config::Config) -> Result<(), String> {
        let mut profiles = Self::list()?;
        for (name, profile) in std::mem::take(&mut config.providers) {
            if profiles.iter().any(|p| p.name == name) {
                return Err(format!(
                    "providers.{name}: name is already saved by tuse connect"
                ));
            }
            profiles.push(profile.into_connect(name));
        }
        config.connect = profiles
            .into_iter()
            .map(|p| {
                p.resolve()
                    .map_err(|e| format!("providers.{}: {e}", p.name))
            })
            .collect::<Result<_, _>>()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_v2_preserves_saved_providers() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE providers (name TEXT PRIMARY KEY, base_url TEXT, api_key TEXT, api TEXT);
            INSERT INTO providers VALUES ('p', 'https://example.com', '$KEY', 'openai-chat');
            PRAGMA user_version = 1;",
        )
        .unwrap();
        for _ in 0..2 {
            initialize_schema(&mut c).unwrap();
        }
        let (key, headers): (String, String) = c
            .query_row(
                "SELECT api_key, headers FROM providers WHERE name = 'p'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((key.as_str(), headers.as_str()), ("$KEY", "{}"));
    }

    #[test]
    fn schema_v3_drops_sessions_and_keeps_providers() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch(SCHEMA).unwrap();
        c.execute_batch(
            "INSERT INTO providers(name, base_url, api_key) VALUES ('p', 'u', 'k');
            INSERT INTO state VALUES ('model', 'm');
            INSERT INTO sessions(id, cwd, title, model, effort, created, updated) VALUES ('s', '/', 't', 'm', 'e', '', '');
            INSERT INTO history VALUES ('s', 1, '{}');
            PRAGMA user_version = 2;",
        )
        .unwrap();
        initialize_schema(&mut c).unwrap();
        let count = |t: &str| -> i64 {
            c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(
            [
                count("sessions"),
                count("history"),
                count("providers"),
                count("state")
            ],
            [0, 0, 1, 1]
        );
    }

    #[test]
    fn schema_reset_discards_legacy_data() {
        let dir = std::env::temp_dir().join(format!("tuse-db-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tuse.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE providers (name TEXT PRIMARY KEY, base_url TEXT NOT NULL, api_key TEXT NOT NULL);
                 INSERT INTO providers VALUES ('old', 'http://127.0.0.1:1', 'k');
                 CREATE TABLE sessions (id TEXT PRIMARY KEY, meta TEXT NOT NULL);
                 INSERT INTO sessions VALUES ('old', '{}');
                 CREATE TABLE records (session TEXT, seq INTEGER, body TEXT);
                 INSERT INTO records VALUES ('old', 1, '{}');",
            )
            .unwrap();
        }
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let p = path.clone();
                let b = barrier.clone();
                std::thread::spawn(move || {
                    let mut c = Connection::open(&p).unwrap();
                    c.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
                    b.wait();
                    initialize_schema(&mut c).unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let c = Connection::open(&path).unwrap();
        for table in ["sessions", "history", "providers", "state"] {
            let count: i64 = c
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
