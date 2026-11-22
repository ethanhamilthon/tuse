use crate::session::Session;
use crate::tools;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use tuse_py::{Adopt, Args, Ctx, Fail, Host, MontyObject, Python};

const FUNCTIONS: &[&str] = &[
    "read",
    "write",
    "edit",
    "grep",
    "task_run",
    "task_add",
    "task_get",
    "task_stop",
    "task_list",
    "docs",
    "set_title",
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
            "set_title" => {
                let text: String = a
                    .str(0, "text")?
                    .trim()
                    .replace(['\r', '\n'], " ")
                    .chars()
                    .take(80)
                    .collect();
                if text.is_empty() {
                    return Err(Fail::runtime("empty title"));
                }
                let mut st = s.state.lock().unwrap();
                st.meta.title = text;
                s.save_meta(&mut st);
                Ok(MontyObject::none())
            }
            "docs" => {
                let topic = a.opt_str(0, "topic")?;
                let mut text = crate::docs::docs(topic.as_deref());
                if topic.as_deref().is_some_and(|t| {
                    matches!(
                        t.trim().to_ascii_lowercase().as_str(),
                        "functions" | "functions.md"
                    )
                }) {
                    text.push_str(&crate::helpers::listing(&s.config(), s.mode, true));
                }
                Ok(MontyObject::string(text))
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

pub(crate) fn python_for(session: Weak<Session>, config: &tuse_config::Config) -> Python {
    let s = session.upgrade();
    let mut py = Python::new(
        Arc::new(SessionHost::new(session)),
        config.settings.max_output,
    );
    if let Some(s) = s {
        crate::helpers::install(&s, &mut py, config);
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
