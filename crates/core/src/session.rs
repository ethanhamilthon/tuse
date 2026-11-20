use crate::control::InputId;
use crate::event::Event;
use crate::history::{self, Bounds};
use crate::hooks::Data;
use crate::record::{Record, UserInput};
use crate::store::{Meta, SessionDir};
use crate::tasks::Tasks;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tuse_config::Config;
use tuse_py::Python;

/// One conversation. A process holds at most one open session, and a session
/// can be open in one process only (`SessionDir` holds a file lock).
pub struct Session {
    pub id: String,
    pub cwd: PathBuf,
    pub headless: bool,
    pub tasks: Tasks,
    pub(crate) dir: Mutex<Option<SessionDir>>,
    pub(crate) config: RwLock<Arc<Config>>,
    pub(crate) config_arg: Option<PathBuf>,
    pub(crate) state: Mutex<State>,
    pub(crate) subs: Mutex<Vec<Sender<Event>>>,
    pub(crate) asks: Mutex<HashMap<String, Sender<String>>>,
    pub(crate) ask_seq: AtomicUsize,
    /// The agent's interpreter; made on the first call, gone with the process.
    pub(crate) python: Mutex<Option<Python>>,
    pub(crate) remote: Mutex<Vec<String>>,
    pub(crate) prints: Mutex<HashSet<String>>,
    pub(crate) http: tuse_api::HttpAgent,
}

pub(crate) struct State {
    pub records: Vec<Record>,
    pub bounds: Bounds,
    pub model: String,
    pub effort: String,
    pub busy: bool,
    pub generation: u64,
    pub cancel: Arc<AtomicBool>,
    pub queue: Vec<(InputId, UserInput)>,
    pub next_input: u64,
    pub turn_start: Instant,
    pub calls: usize,
    /// What the agent is doing now; the working indicator shows it.
    pub status: String,
    pub context: u64,
    pub meta: Meta,
}

pub struct Options {
    pub cwd: PathBuf,
    /// `--config PATH`; without it the config is searched for on every (re)load.
    pub config: Option<PathBuf>,
    pub headless: bool,
    /// Resume this session instead of creating a new one.
    pub resume: Option<String>,
}

impl Session {
    pub fn open(opts: Options) -> Result<Arc<Session>, String> {
        let config = load_config(opts.config.as_deref(), &opts.cwd)?;
        let id = opts
            .resume
            .clone()
            .unwrap_or_else(crate::id::new_session_id);
        if opts.resume.is_some() && !crate::store::session_path(&id).join("meta.json").is_file() {
            return Err(format!("no session {id}"));
        }
        let mut dir = SessionDir::open(&id)?;
        let mut records = dir.records();
        for r in history::missing_results(&records) {
            dir.append(&r);
            records.push(r);
        }
        let meta = crate::store::read_meta(&dir.path).unwrap_or_else(|| Meta {
            id: id.clone(),
            cwd: opts.cwd.display().to_string(),
            created: crate::id::now(),
            ..Default::default()
        });
        if opts.resume.is_none() {
            let r = crate::control::initial_settings(&config);
            dir.append(&r);
            records.push(r);
        }
        let mut st = State {
            bounds: Bounds::default(),
            model: crate::control::or_default(&meta.model, config.default_model()),
            effort: crate::control::or_default(&meta.effort, config.default_effort()),
            busy: false,
            generation: 0,
            cancel: Arc::default(),
            queue: vec![],
            next_input: 0,
            turn_start: Instant::now(),
            calls: 0,
            status: String::new(),
            context: 0,
            meta,
            records: vec![],
        };
        for r in &records {
            match r {
                Record::Compaction { prune, drop } => {
                    st.bounds = Bounds {
                        prune: *prune,
                        drop: *drop,
                    }
                }
                Record::Settings { model, effort } => {
                    st.model = model.clone();
                    st.effort = effort.clone();
                }
                Record::Usage { usage, .. } => st.context = usage.input + usage.output,
                _ => {}
            }
        }
        st.records = records;
        let path = dir.path.clone();
        let cwd = PathBuf::from(&st.meta.cwd);
        let s = Arc::new(Session {
            id,
            cwd: if opts.resume.is_some() && cwd.is_dir() {
                cwd
            } else {
                opts.cwd
            },
            headless: opts.headless,
            tasks: Tasks::new(path.join("tasks")),
            dir: Mutex::new(Some(dir)),
            config: RwLock::new(Arc::new(config)),
            config_arg: opts.config,
            state: Mutex::new(st),
            subs: Mutex::default(),
            asks: Mutex::default(),
            ask_seq: AtomicUsize::new(0),
            python: Mutex::default(),
            remote: Mutex::default(),
            prints: Mutex::default(),
            http: tuse_api::default_agent(),
        });
        s.start()?;
        Ok(s)
    }

    fn start(self: &Arc<Self>) -> Result<(), String> {
        self.hook("session_start", Data::default());
        Ok(())
    }

    /// Stops the turn and tasks, fires `session_end` (and waits for those hooks)
    /// and releases the lock.
    pub fn close(self: &Arc<Self>) {
        if self.dir.lock().unwrap().is_none() {
            return;
        }
        self.cancel();
        self.tasks.stop_all();
        self.asks.lock().unwrap().clear();
        for h in self.hook("session_end", Data::default()) {
            let _ = h.join();
        }
        self.dir.lock().unwrap().take();
    }

    pub fn config(&self) -> Arc<Config> {
        self.config.read().unwrap().clone()
    }

    /// Rebuilds the config from its files.
    pub fn reload(&self) -> Result<(), String> {
        let c = load_config(self.config_arg.as_deref(), &self.cwd)?;
        *self.config.write().unwrap() = Arc::new(c);
        Ok(())
    }

    pub(crate) fn emit(&self, e: Event) {
        self.subs
            .lock()
            .unwrap()
            .retain(|tx| tx.send(e.clone()).is_ok());
    }

    pub(crate) fn push(&self, st: &mut State, r: Record) {
        if let Some(d) = self.dir.lock().unwrap().as_mut() {
            d.append(&r);
        }
        st.records.push(r);
    }

    pub(crate) fn save_meta(&self, st: &mut State) {
        st.meta.model = st.model.clone();
        st.meta.effort = st.effort.clone();
        st.meta.updated = crate::id::now();
        if let Some(d) = self.dir.lock().unwrap().as_ref() {
            d.write_meta(&st.meta);
        }
    }

    /// Environment of `sh`, tasks and `os.environ`.
    pub fn env(&self) -> Vec<(String, String)> {
        let model = self.state.lock().unwrap().model.clone();
        let mut env = vec![
            ("TUSE_SESSION".into(), self.id.clone()),
            ("TUSE_CWD".into(), self.cwd.display().to_string()),
            ("TUSE_MODEL".into(), model),
        ];
        if let Some(c) = tuse_config::find(self.config_arg.as_deref(), &self.cwd) {
            env.push(("TUSE_CURRENT_CONFIG".into(), c.display().to_string()));
        }
        env
    }
}

fn load_config(arg: Option<&Path>, cwd: &Path) -> Result<Config, String> {
    let path = tuse_config::find(arg, cwd);
    tuse_config::load(path.as_deref()).map(|l| l.config)
}
