use crate::event::Event;
use crate::record::UserInput;
use crate::session::Session;
use crate::tasks::{self, TaskInfo};
use std::fs::File;
use std::sync::Arc;
use tuse_py::{Adopt, Args, Fail, MontyObject, opt_int, record};

/// `task_run`, `task_add`, `task_get`, `task_stop` and `task_list`.
pub fn call(s: &Arc<Session>, name: &str, a: &Args) -> Result<MontyObject, Fail> {
    match name {
        "task_run" => {
            let t = run(s, &a.str(0, "code")?, a.opt_str(1, "name")?)?;
            Ok(task(&t, ""))
        }
        "task_add" => {
            let (id, code) = (a.str(0, "id")?, a.str(1, "code")?);
            let place = s.tasks.add(&id, &code).map_err(Fail::value)?;
            s.emit(Event::Tasks);
            Ok(MontyObject::int(place as i64))
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
        _ => Err(Fail::runtime(format!("unknown function {name}"))),
    }
}

fn task(t: &TaskInfo, output: &str) -> MontyObject {
    record(
        "Task",
        vec![
            ("id", MontyObject::string(&t.id)),
            ("name", MontyObject::string(&t.name)),
            ("kind", MontyObject::string(&t.kind)),
            ("code", MontyObject::string(&t.code)),
            ("status", MontyObject::string(&t.status)),
            ("exit", opt_int(t.exit)),
            ("queue", MontyObject::int(t.queue as i64)),
            ("output", MontyObject::string(output)),
        ],
    )
}

/// What the agent hears when a task ends or its REPL goes idle.
fn notify(s: &Arc<Session>) -> tasks::Notify {
    let weak = Arc::downgrade(s);
    Arc::new(move |t: TaskInfo, last: String| {
        let Some(s) = weak.upgrade() else { return };
        s.emit(Event::Tasks);
        let what = if t.status == "idle" {
            "is idle".to_string()
        } else {
            let exit = t
                .exit
                .map_or("none (killed)".to_string(), |c| c.to_string());
            format!("exited with code {exit}")
        };
        s.submit(UserInput::notification(format!(
            "<notification>Task {} ({}) {what}. Last line: {last}. Output: `task_get(\"{}\").output`.</notification>",
            t.id, t.name, t.id
        )));
    })
}

fn run(s: &Arc<Session>, code: &str, name: Option<String>) -> Result<TaskInfo, Fail> {
    let config = s.config();
    let weak = Arc::downgrade(s);
    let make = Box::new(move || crate::host::python_for(weak, &config, false));
    let r = s.tasks.run_repl(code, name, &s.cwd, make, notify(s));
    s.emit(Event::Tasks);
    r.map_err(Fail::runtime)
}

/// A timed out `sh` goes on as a process task.
pub fn adopt(s: &Arc<Session>, a: Adopt) -> Result<(String, File), Adopt> {
    let Adopt { child, cmd, cwd } = a;
    match s.tasks.adopt(child, &cmd, &cwd, notify(s)) {
        Ok(r) => {
            s.emit(Event::Tasks);
            Ok(r)
        }
        Err(child) => Err(Adopt { child, cmd, cwd }),
    }
}
