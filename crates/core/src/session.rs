use crate::control::{InputId, Subs};
use crate::history::{self, Bounds};
use crate::hooks::Data;
pub use crate::mode::{Mode, Options, Start};
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
    pub mode: Mode,
    pub tasks: Tasks,
    pub(crate) dir: Mutex<Option<SessionDir>>,
    pub(crate) config: RwLock<(u64, Arc<Config>)>,
    pub(crate) config_arg: Option<PathBuf>,
    pub(crate) state: Mutex<State>,
    pub(crate) subs: Mutex<Subs>,
    pub(crate) ask_seq: AtomicUsize,
    /// The agent's interpreter; made on the first call, gone with the process.
    pub(crate) hooks: Mutex<Option<HashMap<String, crate::hook_queue::HookQueue>>>,
    pub(crate) python: Mutex<Option<Python>>,
    pub(crate) http: tuse_api::HttpAgent,
}

pub(crate) struct State {
    pub records: Vec<Record>,
    pub bounds: Bounds,
    pub model: String,
    pub effort: String,
    pub busy: bool,
    pub turn_error: Option<String>,
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
    /// Assistant text streamed in the current step; `agent_message` gets it on a cancel.
    pub partial: String,
    /// Open `ask`s and where their answers go.
    pub asks: HashMap<String, Sender<String>>,
    /// IDs of the `draw_ui` blocks.
    pub prints: HashSet<String>,
    /// Models the providers have reported.
    pub remote: Vec<String>,
    /// Totals of `records` and the config whose prices they use.
    pub stats: (crate::usage::Stats, Arc<Config>),
}

impl Session {
    pub fn open(opts: Options) -> Result<Arc<Session>, String> {
        let config = load_config(opts.config.as_deref(), &opts.cwd, opts.mode)?;
        let id = opts
            .resume
            .clone()
            .unwrap_or_else(crate::id::new_session_id);
        if opts.resume.is_some() && !crate::store::session_path(&id).join("meta.json").is_file() {
            return Err(format!("no session {id}"));
        }
        let mut dir = SessionDir::open(&id, opts.resume.is_none())?;
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
            let r = crate::control::initial_settings(&config, opts.mode);
            dir.append(&r);
            records.push(r);
        }
        let mut st = State {
            bounds: Bounds::default(),
            model: crate::control::or_default(&meta.model, config.default_model(opts.mode.into())),
            effort: crate::control::or_default(&meta.effort, config.default_effort()),
            busy: false,
            turn_error: None,
            generation: 0,
            cancel: Arc::default(),
            queue: vec![],
            next_input: 0,
            turn_start: Instant::now(),
            calls: 0,
            status: String::new(),
            context: 0,
            meta,
            stats: Default::default(),
            partial: String::new(),
            asks: HashMap::new(),
            prints: HashSet::new(),
            remote: vec![],
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
        let config = Arc::new(config);
        st.stats = (crate::usage::Stats::of(&records, &config), config.clone());
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
            mode: opts.mode,
            tasks: Tasks::new(path.join("tasks")),
            dir: Mutex::new(Some(dir)),
            config: RwLock::new((0, config)),
            config_arg: opts.config,
            state: Mutex::new(st),
            subs: Mutex::default(),
            ask_seq: AtomicUsize::new(0),
            hooks: Mutex::new(Some(HashMap::new())),
            python: Mutex::default(),
            http: tuse_api::default_agent(),
        });
        s.hook(
            "process_start",
            Data {
                reason: Some(opts.start.reason()),
                ..Default::default()
            },
        );
        Ok(s)
    }

    /// Stops the turn and tasks, fires `process_end` with `reason` (`exit`, `new`,
    /// `resume`), waits for those hooks and releases the lock.
    pub fn close_for(self: &Arc<Self>, reason: &'static str) {
        if self.dir.lock().unwrap().is_none() {
            return;
        }
        self.cancel_turn();
        self.tasks.stop_all();
        self.state.lock().unwrap().asks.clear();
        self.finish_hooks(reason);
        self.hooks.lock().unwrap().take();
        self.dir.lock().unwrap().take();
    }

    pub fn close(self: &Arc<Self>) {
        self.close_for("exit");
    }

    pub fn config(&self) -> Arc<Config> {
        self.config.read().unwrap().1.clone()
    }

    /// Rebuilds the config from its files: `process_end` runs on the old hooks, then
    /// `process_start` on the new ones, each hook in a fresh interpreter.
    pub fn reload(self: &Arc<Self>) -> Result<(), String> {
        let c = Arc::new(load_config(
            self.config_arg.as_deref(),
            &self.cwd,
            self.mode,
        )?);
        self.finish_hooks("reload");
        self.swap_config(c.clone());
        *self.hooks.lock().unwrap() = Some(Default::default());
        if let Some(py) = self.python.lock().unwrap().as_mut() {
            py.set_max_output(c.settings.max_output);
            crate::helpers::install(self, py, &c);
        }
        self.hook(
            "process_start",
            Data {
                reason: Some("reload"),
                ..Default::default()
            },
        );
        Ok(())
    }

    pub(crate) fn push(&self, st: &mut State, r: Record) {
        if let Some(d) = self.dir.lock().unwrap().as_mut() {
            d.append(&r);
        }
        let config = st.stats.1.clone();
        st.stats.0.add(&r, &config);
        st.records.push(r);
    }

    pub(crate) fn save_meta(&self, st: &mut State) {
        st.meta.model = st.model.clone();
        st.meta.effort = st.effort.clone();
        st.meta.updated = crate::id::now();
        if let Some(d) = self.dir.lock().unwrap().as_mut() {
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

fn load_config(arg: Option<&Path>, cwd: &Path, mode: Mode) -> Result<Config, String> {
    let path = tuse_config::find(arg, cwd);
    tuse_config::load(path.as_deref(), mode.into()).map(|l| l.config)
}
