use crate::convo::record::Record;
use crate::convo::usage::Stats;
use crate::event::Event;
use crate::session::bus::Signal;
use crate::session::{Session, State};
use crate::storage::store::{ReadCache, SessionDir};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

impl Session {
    pub(crate) fn stop_commands(&self) {
        for c in self.cmd_runs.lock().unwrap().drain(..) {
            c.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn cached(&self, st: &mut State) -> Arc<ReadCache> {
        st.cache
            .get_or_insert_with(|| {
                Arc::new(SessionDir::read(&self.id).expect("read session history"))
            })
            .clone()
    }

    pub fn records(&self) -> Vec<Record> {
        self.with_records(<[Record]>::to_vec)
    }

    pub fn with_records<T>(&self, f: impl FnOnce(&[Record]) -> T) -> T {
        let cache = self.cached(&mut self.state.lock().unwrap());
        f(&cache.records)
    }

    pub fn history_tag(&self) -> (i64, i64) {
        self.cached(&mut self.state.lock().unwrap()).tag
    }

    pub fn model(&self) -> (String, String) {
        let cache = self.cached(&mut self.state.lock().unwrap());
        (cache.meta.model.clone(), cache.meta.effort.clone())
    }

    pub fn activity(&self) -> crate::Activity {
        self.state.lock().unwrap().activity
    }

    pub(crate) fn set_activity(&self, st: &mut State, activity: crate::Activity) {
        if st.activity != activity {
            st.activity = activity;
            self.wake_ui();
        }
    }

    pub fn busy(&self) -> bool {
        self.state.lock().unwrap().busy
    }

    pub fn turn_info(&self) -> Option<(Duration, String)> {
        let st = self.state.lock().unwrap();
        st.busy
            .then(|| (st.turn_start.elapsed(), st.status.clone()))
    }

    pub fn stats(&self) -> Stats {
        let mut st = self.state.lock().unwrap();
        let mut out = self.cached(&mut st).stats.clone();
        if st.busy {
            out.calls += st.calls;
        }
        out
    }

    pub fn title(&self) -> String {
        self.cached(&mut self.state.lock().unwrap())
            .meta
            .title
            .clone()
    }

    pub(crate) fn lock_live(&self, generation: u64) -> Option<std::sync::MutexGuard<'_, State>> {
        let st = self.state.lock().unwrap();
        (st.generation == generation).then_some(st)
    }

    pub(crate) fn set_status(&self, text: String) {
        let mut st = self.state.lock().unwrap();
        if st.status == text {
            return;
        }
        st.status = text.clone();
        drop(st);
        self.emit(Signal::Status(text));
    }
}

impl Session {
    pub(crate) fn write_history(
        &self,
        st: &mut State,
        meta: Option<&crate::storage::store::Meta>,
        replacement: Option<&[Record]>,
        records: &[Record],
        config: &tuse_config::Config,
    ) -> Result<(), String> {
        let dir = self.dir.lock().unwrap();
        let (meta, seq) =
            dir.as_ref()
                .ok_or("session closed")?
                .write(meta, replacement, records, config)?;
        if let Some(cache) = &mut st.cache {
            let cache = Arc::make_mut(cache);
            cache.tag = (seq, meta.revision);
            cache.meta = meta;
            if let Some(history) = replacement {
                cache.records.clear();
                cache.records.extend_from_slice(history);
                cache.stats = Stats::of(&cache.meta, history);
            } else {
                cache.stats.update_meta(&cache.meta);
            }
            cache.records.extend_from_slice(records);
            cache.stats.add_records(records);
        }
        Ok(())
    }

    pub(crate) fn push(&self, st: &mut State, r: Record) {
        self.record(st, &[r], []);
    }

    pub fn error(&self, text: String) {
        self.message(
            Record::message(crate::Level::Error, text.clone()),
            Event::Error(text),
        );
    }

    pub fn flash(&self, text: String) -> bool {
        self.message(
            Record::message(crate::Level::Flash, text.clone()),
            Event::Flash(text),
        )
    }

    pub(crate) fn message(&self, r: Record, e: Event) -> bool {
        self.record(&mut self.state.lock().unwrap(), &[r], [e])
    }

    pub(crate) fn rebuild_system(
        &self,
        st: &mut State,
        config: &tuse_config::Config,
    ) -> Result<(), String> {
        let text = crate::prompt::system_prompt(config);
        let record = Record::System { text };
        let (meta, seq) = self
            .dir
            .lock()
            .unwrap()
            .as_ref()
            .ok_or("session closed")?
            .system(&record)?;
        if let Some(cache) = &mut st.cache {
            let cache = Arc::make_mut(cache);
            if matches!(cache.records.first(), Some(Record::System { .. })) {
                cache.records[0] = record;
            } else {
                cache.records.insert(0, record);
            }
            cache.tag = (seq, meta.revision);
            cache.meta = meta;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LoadFlags, Mode, Options};

    #[test]
    fn warm_history_matches_sqlite_after_commits_and_rollback() {
        let _env = crate::storage::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("tuse-cache-{}", std::process::id()));
        let old = std::env::var_os("TUSE_HOME");
        unsafe { std::env::set_var("TUSE_HOME", &root) };
        let s = Session::open(Options {
            cwd: root.clone(),
            mode: Mode::Json,
            resume: None,
            start: Default::default(),
            flags: LoadFlags {
                no_global: true,
                no_plugins: true,
            },
        })
        .unwrap();
        let cfg = s.config();
        let mut st = s.state.lock().unwrap();
        drop(s.cached(&mut st));
        let before = Arc::as_ptr(st.cache.as_ref().unwrap());
        let history = vec![
            Record::System {
                text: "base".into()
            };
            512
        ];
        s.write_history(&mut st, None, Some(&history), &[], &cfg)
            .unwrap();
        for _ in 0..8 {
            s.write_history(
                &mut st,
                None,
                None,
                &[Record::message(crate::Level::Notice, "append")],
                &cfg,
            )
            .unwrap();
            assert_eq!(Arc::as_ptr(st.cache.as_ref().unwrap()), before);
        }
        let good = s.cached(&mut st);
        let bad = Record::Usage {
            model: "p/m".into(),
            input: u64::MAX,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            llm: false,
        };
        assert!(s.write_history(&mut st, None, None, &[bad], &cfg).is_err());
        assert!(Arc::ptr_eq(&good, st.cache.as_ref().unwrap()));
        drop(good);
        s.rebuild_system(&mut st, &cfg).unwrap();
        let cached = s.cached(&mut st);
        let stored = SessionDir::read(&s.id).unwrap();
        assert_eq!(cached.tag, stored.tag);
        assert_eq!(
            serde_json::to_value(&cached.records).unwrap(),
            serde_json::to_value(&stored.records).unwrap()
        );
        st.busy = true;
        drop(st);
        let task = s.tasks.create("queued", "").unwrap();
        let (info, elapsed) = s.tasks.end(&task.id, true).unwrap();
        assert!(s.record_task(&info, elapsed, None));
        s.submit_task(crate::convo::record::TaskMessage {
            id: task.id.clone(),
            name: info.name,
            status: info.status,
            delivery: crate::convo::record::TaskDelivery::Wake,
            text: "queued".into(),
        });
        s.cancel_turn();
        let records = s.records();
        assert!(
            records
                .iter()
                .any(|r| matches!(r, Record::Task(job) if job.id.as_ref() == Some(&task.id)))
        );
        assert!(!records.iter().any(|r| matches!(r, Record::TaskMessage(_))));
        let log_dir = std::path::Path::new(&info.log).parent().unwrap();
        let tasks = crate::storage::tasks::Tasks::new(log_dir.to_path_buf());
        assert_ne!(tasks.create("after resume", "").unwrap().id, task.id);
        s.close();
        unsafe {
            if let Some(old) = old {
                std::env::set_var("TUSE_HOME", old)
            } else {
                std::env::remove_var("TUSE_HOME")
            }
        };
        let _ = std::fs::remove_dir_all(root);
    }
}
