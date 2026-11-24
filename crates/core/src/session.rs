use crate::control::{InputId, Subs};
use crate::event::Event;
use crate::history::{self, Bounds};
pub use crate::mode::{Mode, Options, Start};
use crate::record::{Record, UserInput};
use crate::store::{Meta, SessionDir};
use crate::tasks::{TaskInfo, Tasks};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tuse_config::{Config, ModelDef};

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
    pub(crate) vm: tuse_lua::Vm,
    pub(crate) hooks_on: AtomicBool,
    pub(crate) hook_runs: crate::hooks::Runs,
    pub(crate) ui: Mutex<crate::ui::UiState>,
    pub(crate) cmd_runs: Mutex<Vec<Arc<AtomicBool>>>,
    pub(crate) http: tuse_api::HttpAgent,
    pub(crate) next_lua: AtomicU64,
    pub(crate) task_lua: Mutex<HashMap<String, (String, Instant)>>,
    pub(crate) reload_lock: Mutex<()>,
    pub(crate) reload_after_hook: AtomicBool,
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
    pub status: String,
    pub context: u64,
    pub meta: Meta,
    pub partial: String,
    pub remote: Vec<String>,
    pub stats: (crate::usage::Stats, Vec<ModelDef>),
}

pub(crate) struct Prepared {
    opts: Options,
    id: String,
    meta: Meta,
    dir: SessionDir,
    vm: tuse_lua::Vm,
    weak: crate::host::Slot,
    config: Config,
}

impl Session {
    pub fn open(opts: Options) -> Result<Arc<Session>, String> {
        Ok(Session::prepare(opts)?.start())
    }

    pub(crate) fn prepare(mut opts: Options) -> Result<Prepared, String> {
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
        let dir = SessionDir::open(&id, !restored)?;
        let (vm, weak, config) = crate::config_lua::start_vm(&opts)?;
        Ok(Prepared {
            opts,
            id,
            meta,
            dir,
            vm,
            weak,
            config,
        })
    }
}

impl Prepared {
    pub(crate) fn start(self) -> Arc<Session> {
        let Prepared {
            opts,
            id,
            meta,
            mut dir,
            vm,
            weak,
            config,
        } = self;
        let restored = opts.resume.is_some();
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
                Record::Usage {
                    usage, llm: false, ..
                } => st.context = usage.input + usage.output,
                _ => {}
            }
        }
        let ui = crate::ui::UiState::replay(&records);
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
                ui: Mutex::new(ui),
                cmd_runs: Mutex::default(),
                http: tuse_api::default_agent(),
                next_lua: AtomicU64::new(0),
                task_lua: Mutex::default(),
                reload_lock: Mutex::default(),
                reload_after_hook: AtomicBool::new(false),
            }
        });
        s.start_hook(opts.start.reason(), !restored, restored);
        s
    }
}

impl Session {
    pub(crate) fn next_lua_id(&self) -> String {
        let n = self.next_lua.fetch_add(1, Ordering::Relaxed) + 1;
        format!("lua-{n}")
    }

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

    pub fn reload(self: &Arc<Self>) -> Result<(), String> {
        let _one = self.reload_lock.lock().unwrap();
        let path = tuse_config::find(self.config_arg.as_deref(), &self.cwd);
        let c = self
            .vm
            .call(move |lua| crate::config_lua::prepare(lua, path.as_deref(), true))?;
        self.finish_hooks("reload");
        let c = Arc::new(c);
        let (s2, c2) = (self.clone(), c.clone());
        self.vm.call(move |lua| {
            crate::config_lua::commit(lua, &c2)?;
            s2.swap_config(c2);
            Ok::<_, String>(())
        })?;
        self.ui_cancel_hooks(false);
        self.hooks_on.store(true, Ordering::Release);
        self.start_hook("reload", false, false);
        self.emit(crate::Event::Reloaded);
        Ok(())
    }
}

impl Session {
    pub fn is_open(&self) -> bool {
        self.dir.lock().unwrap().is_some()
    }

    pub fn resolve_prompts(
        &self,
        names: &[String],
        strict: bool,
    ) -> Result<Vec<(String, String)>, String> {
        let config = self.config();
        let mut out = vec![];
        for n in names {
            match config.prompts.get(n) {
                Some(body) => out.push((n.clone(), body.clone())),
                None if strict => return Err(format!("unknown prompt `{n}`")),
                None => {}
            }
        }
        Ok(out)
    }

    pub fn stop_task(&self, id: &str) -> Result<TaskInfo, String> {
        let t = self.tasks.stop(id)?;
        self.emit(Event::Tasks);
        Ok(t)
    }

    pub fn switch(self: &Arc<Self>, resume: Option<String>) -> Result<Arc<Session>, String> {
        if resume.as_deref() == Some(self.id.as_str()) {
            return Err(format!("session {} is already open", self.id));
        }
        let start = if resume.is_some() {
            Start::Resume
        } else {
            Start::New
        };
        let next = Session::prepare(Options {
            cwd: self.cwd.clone(),
            config: self.config_arg.clone(),
            mode: self.mode,
            resume,
            start,
        })?;
        self.close_for(start.reason());
        Ok(next.start())
    }

    pub(crate) fn push(&self, st: &mut State, r: Record) {
        if let Some(d) = self.dir.lock().unwrap().as_mut() {
            d.append(&r);
        }
        st.stats.0.add(&r, &st.stats.1);
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
}
