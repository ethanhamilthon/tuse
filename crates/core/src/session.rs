use crate::control::{InputId, Subs};
use crate::history::{self, Bounds};
use crate::hooks::Data;
pub use crate::mode::{Mode, Options, Start};
use crate::record::{Record, UserInput};
use crate::store::{Meta, SessionDir};
use crate::tasks::Tasks;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tuse_config::Config;

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
    /// The VM of the session: config, the agent's code, hooks and tasks.
    pub(crate) vm: tuse_lua::Vm,
    /// Whether hooks still fire; off once the session closes.
    pub(crate) hooks_on: std::sync::atomic::AtomicBool,
    /// Hook runs still going: (hook, event, cancel flag).
    pub(crate) hook_runs: crate::hooks::Runs,
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
    /// Models the providers have reported.
    pub remote: Vec<String>,
    /// Totals of `records` and the config whose prices they use.
    pub stats: (crate::usage::Stats, Arc<Config>),
}

impl Session {
    pub fn open(opts: Options) -> Result<Arc<Session>, String> {
        let id = opts
            .resume
            .clone()
            .unwrap_or_else(crate::id::new_session_id);
        if opts.resume.is_some() && crate::store::read_meta(&id).is_none() {
            return Err(format!("no session {id}"));
        }
        let mut dir = SessionDir::open(&id, opts.resume.is_none())?;
        let (vm, weak, config) = start_vm(&opts)?;
        let mut records = dir.records();
        for r in history::missing_results(&records) {
            dir.append(&r);
            records.push(r);
        }
        let meta = crate::store::read_meta(&id).unwrap_or_else(|| Meta {
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
        let s = Arc::new_cyclic(|me| {
            *weak.lock().unwrap() = me.clone();
            Session {
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
                vm,
                hooks_on: std::sync::atomic::AtomicBool::new(true),
                hook_runs: Default::default(),
                http: tuse_api::default_agent(),
            }
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
        self.finish_hooks(reason);
        self.dir.lock().unwrap().take();
    }

    pub fn close(self: &Arc<Self>) {
        self.close_for("exit");
    }

    pub fn config(&self) -> Arc<Config> {
        self.config.read().unwrap().1.clone()
    }

    /// Runs the config file again in the same VM (the agent's globals stay):
    /// `process_end` runs on the old hooks, then `process_start` on the new ones.
    pub fn reload(self: &Arc<Self>) -> Result<(), String> {
        let path = tuse_config::find(self.config_arg.as_deref(), &self.cwd);
        let mode = self.mode.into();
        let c = self
            .vm
            .call(move |lua| crate::config_lua::prepare(lua, path.as_deref(), mode))?;
        self.finish_hooks("reload");
        let c = Arc::new(c);
        let c2 = c.clone();
        self.vm
            .call(move |lua| crate::config_lua::commit(lua, &c2, mode))?;
        self.swap_config(c);
        self.hooks_on
            .store(true, std::sync::atomic::Ordering::Release);
        self.hook(
            "process_start",
            Data {
                reason: Some("reload"),
                ..Default::default()
            },
        );
        Ok(())
    }
}

/// The session's VM with the config loaded. The session is not built yet, so the VM
/// gets a slot that `open` fills.
fn start_vm(opts: &Options) -> Result<(tuse_lua::Vm, crate::host::Slot, Config), String> {
    let slot = crate::host::Slot::default();
    let vm = tuse_lua::Vm::new(Arc::new(crate::host::SessionHost(slot.clone())), |_| {});
    let path = tuse_config::find(opts.config.as_deref(), &opts.cwd);
    let mode = opts.mode.into();
    let s2 = slot.clone();
    let config = vm.call(move |lua| {
        crate::host::install(lua, s2).map_err(|e| tuse_lua::message(&e))?;
        crate::config_lua::load(lua, path.as_deref(), mode)
    })?;
    Ok((vm, slot, config))
}
