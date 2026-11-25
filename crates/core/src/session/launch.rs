use crate::convo::record::Record;
use crate::convo::wire;
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
    dir: SessionDir,
    vm: tuse_lua::Vm,
    weak: crate::session::lua::host::Slot,
    config: Config,
}

pub fn open(opts: Options) -> Result<Arc<Session>, String> {
    prepare(opts)?.start()
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
    session.close_for(Some("session_close"));
    next.start()
}

fn prepare(mut opts: Options) -> Result<Prepared, String> {
    let restored = opts.resume.is_some();
    let id = opts
        .resume
        .clone()
        .unwrap_or_else(crate::id::new_session_id);
    let meta = if restored {
        let meta =
            crate::storage::store::Meta::read(&id)?.ok_or_else(|| format!("no session {id}"))?;
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
            model: crate::storage::Prefs::model(),
            effort: crate::storage::Prefs::effort(),
            ..Default::default()
        }
    };
    let (vm, weak, config) = crate::session::lua::config_lua::start_vm(&opts)?;
    let dir = SessionDir::open(&id, (!restored).then_some(&meta))?;
    Ok(Prepared {
        opts,
        id,
        dir,
        vm,
        weak,
        config,
    })
}

impl Prepared {
    pub(crate) fn start(self) -> Result<Arc<Session>, String> {
        let Prepared {
            opts,
            id,
            dir,
            vm,
            weak,
            config,
        } = self;
        let restored = opts.resume.is_some();
        dir.system(&Record::System {
            text: crate::prompt::system_prompt(&config),
        })?;
        if restored {
            let cache = SessionDir::read(&id)?;
            let missing = wire::missing_results(&cache.records);
            if !missing.is_empty() {
                dir.append(&missing, &config)?;
            }
        }
        let st = State {
            activity: Default::default(),
            cache: None,
            pending_history: None,
            tool_step: false,
            busy: false,
            generation: 0,
            cancel: Arc::default(),
            tool_handoff: None,
            cancel_pending: false,
            queue: vec![],
            next_input: 0,
            turn_start: Instant::now(),
            calls: 0,
            status: String::new(),
            partial: String::new(),
            reasoning: String::new(),
            streaming: false,
            remote: vec![],
        };
        let config = Arc::new(config);
        let path = dir.path();
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
                jobs: Mutex::default(),
                reload_lock: Mutex::default(),
                reload_after_hook: AtomicBool::new(false),
            }
        });
        s.lifecycle(opts.start == Start::Launch, restored);
        Ok(s)
    }
}
