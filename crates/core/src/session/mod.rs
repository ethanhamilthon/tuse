use crate::session::bus::Signal;
pub(crate) mod bus;
pub(crate) mod control;
pub(crate) mod hooks;
pub(crate) mod launch;
pub(crate) mod lua;
pub(crate) mod meta;
pub(crate) mod state;
pub(crate) mod turn;
pub(crate) mod ui;
pub(crate) mod view;

use crate::convo::record::{Record, UserInput};
use crate::event::InputId;
pub use crate::mode::{Mode, Options};
use crate::session::bus::Subs;
use crate::storage::store::{ReadCache, SessionDir};
use crate::storage::tasks::{TaskInfo, Tasks};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tuse_config::Config;

pub struct Session {
    pub id: String,
    pub(crate) cwd: PathBuf,
    pub(crate) mode: Mode,
    pub(crate) tasks: Tasks,
    pub(crate) dir: Mutex<Option<SessionDir>>,
    pub(crate) config: RwLock<(u64, Arc<Config>)>,
    pub(crate) config_arg: Option<PathBuf>,
    pub(crate) state: Mutex<State>,
    pub(crate) subs: Mutex<Subs>,
    pub(crate) vm: tuse_lua::Vm,
    pub(crate) hooks_on: AtomicBool,
    pub(crate) lifecycle_job: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub(crate) hook_runs: crate::session::hooks::lanes::Runs,
    pub(crate) ui: Mutex<crate::session::ui::UiState>,
    pub(crate) cmd_runs: Mutex<Vec<Arc<AtomicBool>>>,
    pub(crate) http: tuse_api::HttpAgent,
    pub(crate) next_lua: AtomicU64,
    pub(crate) task_lua: Mutex<HashMap<String, (String, Instant)>>,
    pub(crate) jobs: Mutex<HashMap<String, (&'static str, String, String)>>,
    pub(crate) reload_lock: Mutex<()>,
    pub(crate) reload_after_hook: AtomicBool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    #[default]
    Idle,
    Provider,
    Retry,
    Lua,
}

pub(crate) struct State {
    pub activity: Activity,
    pub cache: Option<Arc<ReadCache>>,
    pub pending_history: Option<Vec<Record>>,
    pub tool_step: bool,
    pub busy: bool,
    pub generation: u64,
    pub cancel: Arc<AtomicBool>,
    pub tool_handoff: Option<Arc<AtomicBool>>,
    pub cancel_pending: bool,
    pub queue: Vec<(InputId, UserInput)>,
    pub next_input: u64,
    pub turn_start: Instant,
    pub calls: usize,
    pub status: String,
    pub partial: String,
    pub reasoning: String,
    pub streaming: bool,
    pub remote: Vec<String>,
}

impl Session {
    pub fn open(opts: Options) -> Result<Arc<Session>, String> {
        crate::session::launch::open(opts)
    }

    pub fn switch(self: &Arc<Self>, resume: Option<String>) -> Result<Arc<Session>, String> {
        crate::session::launch::switch(self, resume)
    }

    pub fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }

    pub fn tasks(&self) -> Vec<crate::storage::tasks::TaskInfo> {
        self.tasks.list()
    }

    pub fn task(&self, id: &str) -> Option<crate::storage::tasks::TaskInfo> {
        self.tasks.get(id)
    }
}

impl Session {
    pub(crate) fn next_lua_id(&self) -> String {
        let n = self.next_lua.fetch_add(1, Ordering::Relaxed) + 1;
        format!("lua-{n}")
    }

    pub(crate) fn close_for(self: &Arc<Self>, event: Option<&'static str>) {
        if self.dir.lock().unwrap().is_none() {
            return;
        }
        self.ui_close();
        self.stop_commands();
        self.cancel_turn(false);
        self.tasks.stop_all();
        self.finish_hooks(event);
        self.dir.lock().unwrap().take();
    }

    pub fn close(self: &Arc<Self>) {
        self.close_for(Some("process_end"));
    }

    pub fn disconnect(self: &Arc<Self>) {
        self.close_for(None);
    }

    pub fn config(&self) -> Arc<Config> {
        self.config.read().unwrap().1.clone()
    }

    pub fn reload(self: &Arc<Self>) -> Result<(), String> {
        let result = self.reload_config();
        if let Err(e) = &result {
            self.error(format!("Reload failed: {e}"));
        }
        result
    }

    fn reload_config(self: &Arc<Self>) -> Result<(), String> {
        let _one = self.reload_lock.lock().unwrap();
        let path = tuse_config::find(self.config_arg.as_deref(), &self.cwd);
        let c = self
            .vm
            .call(move |lua| crate::session::lua::config_lua::prepare(lua, path.as_deref()))?;
        self.finish_hooks(None);
        let c = Arc::new(c);
        let (s2, c2) = (self.clone(), c.clone());
        self.vm.call(move |lua| {
            crate::session::lua::config_lua::commit(lua)?;
            s2.swap_config(c2)
        })?;
        self.ui_cancel_hooks(false);
        self.hooks_on.store(true, Ordering::Release);
        self.simple_hook("config_load");
        self.emit(Signal::Reloaded);
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
    ) -> Result<Vec<(String, Option<String>, String)>, String> {
        let config = self.config();
        let mut out = vec![];
        for raw in names {
            let (n, arg) = match raw.trim().split_once(char::is_whitespace) {
                Some((n, a)) => (n, Some(a.trim().to_string())),
                None => (raw.trim(), None),
            };
            let Some(p) = config.prompts.get(n) else {
                if strict {
                    return Err(format!("unknown prompt `{n}`"));
                }
                continue;
            };
            if let Some(a) = &arg
                && !p.args.contains(a)
            {
                return Err(format!("prompt `{n}`: unknown arg `{a}`"));
            }
            let body = match p.kind {
                tuse_config::PromptKind::Text => p.body.clone(),
                tuse_config::PromptKind::Function => self.prompt_body(n, arg.as_deref())?,
            };
            out.push((n.to_string(), arg, body));
        }
        Ok(out)
    }

    pub fn stop_task(&self, id: &str) -> Result<TaskInfo, String> {
        let t = self.tasks.stop(id)?;
        self.emit(Signal::Tasks);
        Ok(t)
    }
}
