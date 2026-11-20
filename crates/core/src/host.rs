use crate::event::Event;
use crate::session::Session;
use crate::tools;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use tuse_py::{Adopt, Args, Ctx, Fail, Host, MontyObject, NEVER, Python};

const FUNCTIONS: &[&str] = &[
    "read",
    "write",
    "edit",
    "grep",
    "ask",
    "show",
    "task_run",
    "task_add",
    "task_get",
    "task_stop",
    "task_list",
    "docs",
];

/// The Python functions that reach into a session. Holds the session weakly:
/// the session owns the interpreter that owns this.
pub(crate) struct SessionHost {
    session: Weak<Session>,
}

impl SessionHost {
    pub fn new(session: Weak<Session>) -> Self {
        Self { session }
    }

    fn session(&self) -> Result<Arc<Session>, Fail> {
        self.session
            .upgrade()
            .ok_or_else(|| Fail::runtime("session closed"))
    }
}

impl Host for SessionHost {
    fn functions(&self) -> &'static [&'static str] {
        FUNCTIONS
    }

    fn call(&self, name: &str, a: &Args, ctx: &Ctx) -> Result<MontyObject, Fail> {
        let s = self.session()?;
        match name {
            "read" | "write" | "edit" | "grep" => tools::call(name, a, ctx, &s.cwd, &s.config()),
            "ask" => ask(&s, a, ctx),
            "show" => {
                let id = s
                    .print(a.str(0, "text")?, a.opt_str(1, "id")?)
                    .map_err(Fail::runtime)?;
                Ok(MontyObject::string(id))
            }
            "docs" => {
                let topic = a.opt_str(0, "topic")?;
                Ok(MontyObject::string(crate::docs::docs(topic.as_deref())))
            }
            _ => crate::host_tasks::call(&s, name, a),
        }
    }

    fn cwd(&self) -> PathBuf {
        self.session()
            .map_or_else(|_| PathBuf::from("/"), |s| s.cwd.clone())
    }

    fn env(&self) -> Vec<(String, String)> {
        self.session().map(|s| s.env()).unwrap_or_default()
    }

    fn module_dirs(&self) -> Vec<PathBuf> {
        let Ok(s) = self.session() else { return vec![] };
        tuse_config::find(s.config_arg.as_deref(), &s.cwd)
            .and_then(|p| p.parent().map(PathBuf::from))
            .into_iter()
            .collect()
    }

    fn adopt(&self, a: Adopt) -> Result<(String, File), Adopt> {
        match self.session() {
            Ok(s) => crate::host_tasks::adopt(&s, a),
            Err(_) => Err(a),
        }
    }
}

/// Blocks until the user answers. The time it takes does not count against the
/// deadline of the call.
fn ask(s: &Arc<Session>, a: &Args, ctx: &Ctx) -> Result<MontyObject, Fail> {
    a.deny(2, "wait")?;
    let question = a.str(0, "question")?;
    let options = a.str_list(1, "options")?;
    let (id, rx) = s.ask(&question, options).map_err(Fail::runtime)?;
    let _paused = ctx.pause();
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(answer) => return Ok(MontyObject::string(answer)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(Fail::runtime("dismissed"));
            }
            Err(_) if ctx.cancelled() => {
                s.answer(&id, None);
                return Err(Fail::runtime("stopped before the user answered"));
            }
            Err(_) => {}
        }
    }
}

/// A fresh interpreter with the configured `prelude` already run.
pub(crate) fn python_for(session: Weak<Session>, config: &tuse_config::Config) -> Python {
    let host = Arc::new(SessionHost::new(session.clone()));
    let mut py = Python::new(host, config.settings.max_output);
    let prelude = config.prelude.join("\n");
    if !prelude.trim().is_empty() {
        let secs = config.settings.hook_timeout.max(1);
        let ctx = Ctx::new(&NEVER, Instant::now() + Duration::from_secs(secs));
        let o = py.run(&prelude, &ctx, vec![]);
        if !o.ok
            && let Some(s) = session.upgrade()
        {
            s.emit(Event::Notice(format!("prelude: {}", last_line(&o.text))));
        }
    }
    py
}

impl Session {
    /// The agent's interpreter.
    pub(crate) fn new_python(self: &Arc<Self>, config: &tuse_config::Config) -> Python {
        python_for(Arc::downgrade(self), config)
    }
}

pub(crate) fn last_line(text: &str) -> String {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("failed")
        .to_string()
}
