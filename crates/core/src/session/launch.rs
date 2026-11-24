use crate::convo::history::{self, Bounds};
use crate::convo::record::Record;
use crate::mode::{Options, Start};
use crate::session::{Session, State};
use crate::storage::store::{Meta, SessionDir};
use crate::storage::tasks::Tasks;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tuse_config::Config;

pub(crate) struct Prepared {
    opts: Options,
    id: String,
    meta: Meta,
    dir: SessionDir,
    vm: tuse_lua::Vm,
    weak: crate::session::lua::host::Slot,
    config: Config,
}

pub fn open(opts: Options) -> Result<Arc<Session>, String> {
    Ok(prepare(opts)?.start())
}

pub fn switch(session: &Arc<Session>, resume: Option<String>) -> Result<Arc<Session>, String> {
    if resume.as_deref() == Some(session.id.as_str()) {
        return Err(format!("session {} is already open", session.id));
    }
    let start = if resume.is_some() {
        Start::Resume
    } else {
        Start::New
    };
    let next = prepare(Options {
        cwd: session.cwd.clone(),
        config: session.config_arg.clone(),
        mode: session.mode,
        resume,
        start,
    })?;
    session.close_for(start.reason());
    Ok(next.start())
}

fn prepare(mut opts: Options) -> Result<Prepared, String> {
    let restored = opts.resume.is_some();
    let id = opts
        .resume
        .clone()
        .unwrap_or_else(crate::convo::id::new_session_id);
    let meta = if restored {
        let meta =
            crate::storage::store::read_meta(&id).ok_or_else(|| format!("no session {id}"))?;
        let cwd = PathBuf::from(&meta.cwd);
        if cwd.is_dir() {
            opts.cwd = cwd;
        }
        meta
    } else {
        Meta {
            id: id.clone(),
            cwd: opts.cwd.display().to_string(),
            created: crate::convo::id::now(),
            ..Default::default()
        }
    };
    let dir = SessionDir::open(&id, !restored)?;
    let (vm, weak, config) = crate::session::lua::config_lua::start_vm(&opts)?;
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
        let default_model = crate::storage::db::default_model();
        let default_effort = crate::storage::db::default_effort();
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
            model: crate::session::state::or_default(&meta.model, default_model),
            effort: crate::session::state::or_default(&meta.effort, default_effort),
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
        let ui = crate::session::ui::UiState::replay(&records);
        let models = config.models.clone();
        st.stats = (crate::convo::usage::Stats::of(&records, &models), models);
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
