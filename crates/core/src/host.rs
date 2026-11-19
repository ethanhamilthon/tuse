use crate::event::Event;
use crate::record::UserInput;
use crate::session::Session;
use crate::tasks::{self, TaskInfo};
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use tuse_py::{Args, Ctx, Fail, Host, MontyObject, NEVER, Python, opt_int, record};

const FUNCTIONS: &[&str] = &[
    "ask",
    "show",
    "task_run",
    "task_get",
    "task_stop",
    "task_list",
    "docs",
];

/// The Python functions that reach into a session. Holds the session weakly:
/// the session owns the interpreter that owns this.
pub(crate) struct SessionHost(pub Weak<Session>);

impl SessionHost {
    fn session(&self) -> Result<Arc<Session>, Fail> {
        self.0
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
            "ask" => ask(&s, a, ctx),
            "show" => {
                let id = s
                    .print(a.str(0, "text")?, a.opt_str(1, "id")?)
                    .map_err(Fail::runtime)?;
                Ok(MontyObject::string(id))
            }
            "task_run" => {
                let t = run_task(&s, &a.str(0, "cmd")?, a.opt_str(1, "name")?)?;
                Ok(task(&t, ""))
            }
            "task_get" => {
                let id = a.str(0, "id")?;
                let t = s
                    .tasks
                    .get(&id)
                    .ok_or_else(|| Fail::value(format!("no task {id}")))?;
                let lines = a.opt_int(1, "lines")?.map_or(50, |n| n.max(1) as usize);
                Ok(task(&t, &tasks::tail(&t.log, lines)))
            }
            "task_stop" => {
                let r = s.tasks.stop(&a.str(0, "id")?);
                s.emit(Event::Tasks);
                r.map(|t| task(&t, "")).map_err(Fail::value)
            }
            "task_list" => Ok(MontyObject::list(
                s.tasks.list().iter().map(|t| task(t, "")),
            )),
            "docs" => Ok(MontyObject::string(crate::docs::docs())),
            _ => Err(Fail::runtime(format!("unknown function {name}"))),
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
}

fn task(t: &TaskInfo, output: &str) -> MontyObject {
    record(
        "Task",
        vec![
            ("id", MontyObject::string(&t.id)),
            ("name", MontyObject::string(&t.name)),
            ("cmd", MontyObject::string(&t.command)),
            ("status", MontyObject::string(&t.status)),
            ("code", opt_int(t.code)),
            ("output", MontyObject::string(output)),
        ],
    )
}

fn run_task(s: &Arc<Session>, cmd: &str, name: Option<String>) -> Result<TaskInfo, Fail> {
    let weak = Arc::downgrade(s);
    let on_exit = Box::new(move |t: TaskInfo| {
        if let Some(s) = weak.upgrade() {
            s.emit(Event::Tasks);
            let code = t
                .code
                .map_or("none (killed)".to_string(), |c| c.to_string());
            s.submit(UserInput::text(format!(
                "<notification>Task {} ({}) exited with code {code}. Output: `task_get(\"{}\").output`.</notification>",
                t.id, t.name, t.id
            )));
        }
    });
    let r = s.tasks.run(cmd, name, &s.cwd, &s.env(), on_exit);
    s.emit(Event::Tasks);
    r.map_err(Fail::runtime)
}

fn ask(s: &Arc<Session>, a: &Args, ctx: &Ctx) -> Result<MontyObject, Fail> {
    let question = a.str(0, "question")?;
    let options = a.str_list(1, "options")?;
    let wait = a.bool(2, "wait", true)?;
    let (id, rx) = s.ask(&question, options).map_err(Fail::runtime)?;
    if !wait {
        let s = s.clone();
        let ask_id = id.clone();
        std::thread::spawn(move || {
            let answer = rx.recv().unwrap_or_else(|_| "(dismissed)".into());
            s.submit(UserInput::text(format!(
                "<notification>Answer to ask {ask_id} \"{question}\": {answer}</notification>"
            )));
        });
        return Ok(MontyObject::string(id));
    }
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(answer) => return Ok(MontyObject::string(answer)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(Fail::runtime("dismissed"));
            }
            Err(_) if ctx.stopped() => {
                s.answer(&id, None);
                return Err(Fail::runtime("stopped before the user answered"));
            }
            Err(_) => {}
        }
    }
}

impl Session {
    /// A fresh interpreter with the configured `prelude` already run.
    pub(crate) fn new_python(self: &Arc<Self>, config: &tuse_config::Config) -> Python {
        let host = Arc::new(SessionHost(Arc::downgrade(self)));
        let mut py = Python::new(host, config.settings.max_output);
        let prelude = config.prelude.join("\n");
        if !prelude.trim().is_empty() {
            let secs = config.settings.tool_timeout.max(1);
            let ctx = Ctx {
                cancel: &NEVER,
                deadline: Instant::now() + Duration::from_secs(secs),
            };
            let o = py.run(&prelude, &ctx, vec![]);
            if !o.ok {
                self.emit(Event::Notice(format!("prelude: {}", last_line(&o.text))));
            }
        }
        py
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
