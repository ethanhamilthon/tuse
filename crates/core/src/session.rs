use crate::control::{InputId, Subs};
use crate::history::{self, Bounds};
pub use crate::mode::{Mode, Options, Start};
use crate::record::{Record, UserInput};
use crate::store::{Meta, SessionDir};
use crate::tasks::Tasks;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tuse_config::{Config, ModelDef};

/// One conversation. A process holds at most one open session, and a session
/// can be open in one process only (`SessionDir` holds a file lock).
pub struct Session {
    pub id: String,
    pub cwd: PathBuf,
    pub(crate) mode: Mode,
    pub tasks: Tasks,
    pub(crate) dir: Mutex<Option<SessionDir>>,
    pub(crate) config: RwLock<(u64, Arc<Config>)>,
    pub(crate) config_arg: Option<PathBuf>,
    pub(crate) state: Mutex<State>,
    pub(crate) subs: Mutex<Subs>,
    /// The VM of the session: config, the agent's code, hooks and tasks.
    pub(crate) vm: tuse_lua::Vm,
    /// Whether hooks still fire; off once the session closes.
    pub(crate) hooks_on: AtomicBool,
    /// Hook runs still going: per-hook lane.
    pub(crate) hook_runs: crate::hooks::Runs,
    pub(crate) ui: Mutex<crate::ui::UiState>,
    pub(crate) cmd_runs: Mutex<Vec<Arc<AtomicBool>>>,
    pub(crate) http: tuse_api::HttpAgent,
    pub(crate) next_lua: AtomicU64,
    pub(crate) task_lua: Mutex<HashMap<String, (String, Instant)>>,
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
    /// Models the providers have reported.
    pub remote: Vec<String>,
    /// Totals of `records` and the model defs whose prices they use.
    pub stats: (crate::usage::Stats, Vec<ModelDef>),
}

impl Session {
    pub fn open(mut opts: Options) -> Result<Arc<Session>, String> {
        let restored = opts.resume.is_some();
        let id = opts
            .resume
            .clone()
            .unwrap_or_else(crate::id::new_session_id);
        let meta = if restored {
            let meta = crate::store::read_meta(&id).ok_or_else(|| format!("no session {id}"))?;
            let cwd = PathBuf::from(&meta.cwd);
            if cwd.is_dir() {
                opts.cwd = cwd;
            }
            meta
        } else {
            Meta {
                id: id.clone(),
                cwd: opts.cwd.display().to_string(),
                created: crate::id::now(),
                ..Default::default()
            }
        };
        let mut dir = SessionDir::open(&id, !restored)?;
        let (vm, weak, config) = crate::config_lua::start_vm(&opts)?;
        let mut records = dir.records();
        for r in history::missing_results(&records) {
            dir.append(&r);
            records.push(r);
        }
        let default_model = crate::db::default_model();
        let default_effort = crate::db::default_effort();
        if !restored {
            let r = Record::Settings {
                model: default_model.clone(),
                effort: default_effort.clone(),
            };
            dir.append(&r);
            records.push(r);
        }
        let mut st = State {
            bounds: Bounds::default(),
            model: crate::control::or_default(&meta.model, default_model),
            effort: crate::control::or_default(&meta.effort, default_effort),
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
        let models = config.models.clone();
        st.stats = (crate::usage::Stats::of(&records, &models), models);
        st.records = records;
        let config = Arc::new(config);
        let path = dir.path.clone();
        let s = Arc::new_cyclic(|me| {
            *weak.lock().unwrap() = me.clone();
            Session {
                id,
                cwd: opts.cwd,
                mode: opts.mode,
                tasks: Tasks::new(path.join("tasks")),
                dir: Mutex::new(Some(dir)),
                config: RwLock::new((0, config)),
                config_arg: opts.config,
                state: Mutex::new(st),
                subs: Mutex::default(),
                vm,
                hooks_on: AtomicBool::new(true),
                hook_runs: Default::default(),
                ui: Mutex::default(),
                cmd_runs: Mutex::default(),
                http: tuse_api::default_agent(),
                next_lua: AtomicU64::new(0),
                task_lua: Mutex::default(),
            }
        });
        s.start_hook(opts.start.reason(), !restored, restored);
        Ok(s)
    }

    pub(crate) fn next_lua_id(&self) -> String {
        let n = self.next_lua.fetch_add(1, Ordering::Relaxed) + 1;
        format!("lua-{n}")
    }

    /// Stops the turn and tasks, fires `close` with `reason` (`exit`, `new`, `resume`),
    /// waits for those hooks while keeping the session readable, and releases the lock.
    pub fn close_for(self: &Arc<Self>, reason: &'static str) {
        if self.dir.lock().unwrap().is_none() {
            return;
        }
        self.ui_close();
        self.stop_commands();
        self.cancel_turn();
        self.tasks.stop_all();
        self.finish_hooks(reason);
        self.dir.lock().unwrap().take();
    }

    pub fn close(self: &Arc<Self>) {
        self.close_for("exit");
    }

    pub fn config(&self) -> Arc<Config> {
        self.config.read().unwrap().1.clone()
    }

    /// Runs and validates the config file again in the same VM (agent globals stay):
    /// if validation fails, the previous config and callbacks stay active.
    /// Otherwise `close` runs on old hooks, then `start` (`reason = "reload"`) on new hooks.
    pub fn reload(self: &Arc<Self>) -> Result<(), String> {
        let path = tuse_config::find(self.config_arg.as_deref(), &self.cwd);
        let c = self
            .vm
            .call(move |lua| crate::config_lua::prepare(lua, path.as_deref()))?;
        self.finish_hooks("reload");
        let c = Arc::new(c);
        let c2 = c.clone();
        self.vm
            .call(move |lua| crate::config_lua::commit(lua, &c2))?;
        self.swap_config(c);
        self.ui_cancel_hooks(false);
        self.hooks_on.store(true, Ordering::Release);
        self.start_hook("reload", false, false);
        Ok(())
    }
}
