use crate::convo::{
    record::Record,
    usage::{Stats, cost},
};
use crate::storage::db;
use rusqlite::{OptionalExtension, Row, Transaction, params};
use serde::Serialize;
use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use tuse_api::Usage;
use tuse_config::Config;

const COLS: &str = "id, cwd, title, model, effort, started, created, updated, usage_input,
    usage_output, usage_cached, usage_cache_write, cost, llm_cost, priced, ctx, revision";

#[derive(Debug, Clone, Default, Serialize)]
pub struct Meta {
    pub id: String,
    pub cwd: String,
    pub title: String,
    pub model: String,
    pub effort: String,
    pub started: bool,
    pub created: String,
    pub updated: String,
    #[serde(skip)]
    pub(crate) usage: Usage,
    #[serde(skip)]
    pub(crate) cost: f64,
    #[serde(skip)]
    pub(crate) llm_cost: f64,
    #[serde(skip)]
    pub(crate) priced: bool,
    #[serde(skip)]
    pub(crate) ctx: u64,
    #[serde(skip)]
    pub(crate) revision: i64,
}

pub(crate) struct ReadCache {
    pub meta: Meta,
    pub records: Vec<Record>,
    pub stats: Stats,
    pub tag: (i64, i64),
}

pub struct SessionDir {
    pub id: String,
    _lock: File,
}

impl SessionDir {
    pub fn path(&self) -> PathBuf {
        tuse_config::tuse_home().join(&self.id)
    }

    pub fn open(id: &str, new: Option<&Meta>) -> Result<Self, String> {
        let path = tuse_config::tuse_home().join(id);
        std::fs::create_dir_all(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let lock = File::create(path.join("lock")).map_err(|e| e.to_string())?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(format!("session {id} is open in another tuse process"));
            }
            Err(TryLockError::Error(e)) => return Err(e.to_string()),
        }
        if let Some(m) = new {
            db::with(|c| {
                c.execute(
                "INSERT INTO sessions(id,cwd,title,model,effort,created,updated) VALUES (?1,?2,?3,?4,?5,?6,?6)",
                params![id, m.cwd, m.title, m.model, m.effort, m.created],
            )
            })?;
        }
        Ok(Self {
            id: id.into(),
            _lock: lock,
        })
    }

    pub(crate) fn read(id: &str) -> Result<ReadCache, String> {
        db::with(|c| {
            let tx = c.unchecked_transaction()?;
            let meta = tx.query_row(
                &format!("SELECT {COLS} FROM sessions WHERE id = ?1"),
                [id],
                Meta::decode,
            )?;
            let mut q =
                tx.prepare("SELECT seq, body FROM history WHERE session = ?1 ORDER BY seq")?;
            let mut rows = q.query([id])?;
            let mut seq = 0;
            let mut records = vec![];
            while let Some(r) = rows.next()? {
                seq = r.get(0)?;
                let body: String = r.get(1)?;
                records.push(serde_json::from_str(&body).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?);
            }
            let tag = (seq, meta.revision);
            let stats = Stats::of(&meta, &records);
            Ok(ReadCache {
                meta,
                records,
                stats,
                tag,
            })
        })
    }

    pub(crate) fn append(&self, records: &[Record], config: &Config) -> Result<(), String> {
        self.write(None, None, records, config)
    }

    pub(crate) fn write(
        &self,
        meta: Option<&Meta>,
        replacement: Option<&[Record]>,
        records: &[Record],
        config: &Config,
    ) -> Result<(), String> {
        let id = &self.id;
        db::transaction(|tx| {
            if let Some(m) = meta {
                tx.execute(
                    "UPDATE sessions SET title=?2, model=?3, effort=?4 WHERE id=?1",
                    params![id, m.title, m.model, m.effort],
                )?;
            }
            if let Some(history) = replacement {
                tx.execute("DELETE FROM history WHERE session=?1", [id])?;
                tx.execute("UPDATE sessions SET revision=revision+1 WHERE id=?1", [id])?;
                insert(tx, id, history)?;
            }
            insert(tx, id, records)?;
            for r in records {
                match r {
                    Record::User { .. } => {
                        tx.execute(
                            "UPDATE sessions SET started=1 WHERE id=?1 AND started=0",
                            [id],
                        )?;
                    }
                    Record::Usage {
                        model,
                        usage: u,
                        llm,
                    } => {
                        let price = config.model_def(model).and_then(|m| m.prices.as_ref());
                        let amount = price.map_or(0.0, |p| cost(u, p));
                        tx.execute(
                            "UPDATE sessions SET usage_input=usage_input+?2,
                            usage_output=usage_output+?3, usage_cached=usage_cached+?4,
                            usage_cache_write=usage_cache_write+?5, cost=cost+?6,
                            llm_cost=llm_cost+?7, priced=priced OR ?8,
                            ctx=CASE WHEN ?9 THEN ctx ELSE ?2+?3 END WHERE id=?1",
                            params![
                                id,
                                u.input,
                                u.output,
                                u.cached,
                                u.cache_write,
                                amount,
                                if *llm { amount } else { 0.0 },
                                price.is_some(),
                                llm
                            ],
                        )?;
                    }
                    _ => {}
                }
            }
            tx.execute(
                "UPDATE sessions SET updated=?2 WHERE id=?1",
                params![id, crate::id::now()],
            )?;
            Ok(())
        })
    }

    pub(crate) fn system(&self, record: &Record) -> Result<(), String> {
        db::transaction(|tx| {
            let body = encode(record)?;
            let first: Option<(i64, String)> = tx
                .query_row(
                    "SELECT seq, body FROM history WHERE session=?1 ORDER BY seq LIMIT 1",
                    [&self.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            match first {
                Some((seq, old)) => {
                    if matches!(serde_json::from_str(&old), Ok(Record::System { .. })) {
                        tx.execute(
                            "UPDATE history SET body=?3 WHERE session=?1 AND seq=?2",
                            params![self.id, seq, body],
                        )?;
                    } else {
                        tx.execute(
                            "INSERT INTO history VALUES (?1, ?2, ?3)",
                            params![self.id, seq - 1, body],
                        )?;
                    }
                }
                None => insert(tx, &self.id, std::slice::from_ref(record))?,
            }
            tx.execute(
                "UPDATE sessions SET revision=revision+1 WHERE id=?1",
                [&self.id],
            )?;
            Ok(())
        })
    }
}

fn encode(record: &Record) -> rusqlite::Result<String> {
    serde_json::to_string(record).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn insert(tx: &Transaction<'_>, id: &str, records: &[Record]) -> rusqlite::Result<()> {
    let mut q = tx.prepare_cached(
        "INSERT INTO history(session,seq,body)
        SELECT ?1, COALESCE(MAX(seq),0)+1, ?2 FROM history WHERE session=?1",
    )?;
    for record in records {
        q.execute(params![id, encode(record)?])?;
    }
    Ok(())
}

impl Meta {
    fn decode(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            cwd: r.get(1)?,
            title: r.get(2)?,
            model: r.get(3)?,
            effort: r.get(4)?,
            started: r.get(5)?,
            created: r.get(6)?,
            updated: r.get(7)?,
            usage: Usage {
                input: r.get(8)?,
                output: r.get(9)?,
                cached: r.get(10)?,
                cache_write: r.get(11)?,
            },
            cost: r.get(12)?,
            llm_cost: r.get(13)?,
            priced: r.get(14)?,
            ctx: r.get(15)?,
            revision: r.get(16)?,
        })
    }

    pub(crate) fn read(id: &str) -> Result<Option<Self>, String> {
        db::with(|c| {
            c.query_row(
                &format!("SELECT {COLS} FROM sessions WHERE id=?1"),
                [id],
                Self::decode,
            )
            .optional()
        })
    }

    pub fn list(cwd: &Path) -> Vec<Self> {
        db::with(|c| {
            let mut q = c.prepare(&format!("SELECT {COLS} FROM sessions WHERE cwd=?1 AND started=1 ORDER BY updated DESC, id DESC"))?;
            q.query_map([cwd.display().to_string()], Self::decode)?.collect()
        }).unwrap_or_default()
    }
}
