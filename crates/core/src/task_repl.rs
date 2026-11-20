use crate::tasks::{Live, Notify, REPL, TaskInfo, Tasks};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use tuse_py::{Ctx, Python};

/// A task has no deadline of its own.
const FOREVER: Duration = Duration::from_secs(365 * 24 * 3600);

impl Tasks {
    /// Starts a background Python REPL that runs `code` first. It stays alive
    /// afterwards (`idle`) and runs the snippets of `add` in order.
    pub fn run_repl(
        &self,
        code: &str,
        name: Option<String>,
        cwd: &Path,
        make: Box<dyn FnOnce() -> Python + Send>,
        notify: Notify,
    ) -> Result<TaskInfo, String> {
        let (info, mut log) = self.create(REPL, name, code, cwd, 0)?;
        let (tx, rx) = channel::<String>();
        let cancel = Arc::new(AtomicBool::new(false));
        tx.send(code.to_string()).ok();
        self.live.lock().unwrap().insert(
            info.id.clone(),
            Live {
                tx,
                cancel: cancel.clone(),
            },
        );
        self.update(&info.id, |t| t.queue = 1);
        let (id, tasks) = (info.id.clone(), self.clone());
        std::thread::spawn(move || {
            let mut py = make();
            let mut n = 0;
            while let Ok(snippet) = rx.recv() {
                n += 1;
                tasks.update(&id, |t| {
                    t.queue = t.queue.saturating_sub(1);
                    if t.status == "idle" {
                        t.status = "running".into();
                    }
                });
                if n > 1 {
                    let _ = writeln!(log, "--- task_add #{}", n - 1);
                }
                let ctx = Ctx::new(&cancel, Instant::now() + FOREVER);
                let out = py.run(&snippet, &ctx, vec![]);
                let _ = writeln!(log, "{}", out.text);
                let stopped = cancel.load(std::sync::atomic::Ordering::Relaxed);
                if !py.is_alive() {
                    // A crashed interpreter cannot take more code: end the task.
                    let _ = writeln!(log, "[the interpreter crashed; the task exited]");
                    let done = tasks.update(&id, |t| {
                        t.exit = Some(1);
                        if t.status != "stopped" {
                            t.status = "exited".into();
                        }
                    });
                    if let Some(t) = done {
                        notify(t, "the interpreter crashed".into());
                    }
                    break;
                }
                let Some(t) = tasks.update(&id, |t| {
                    if !stopped && t.queue == 0 {
                        t.status = "idle".into();
                    }
                }) else {
                    break;
                };
                if stopped {
                    break;
                }
                if t.queue == 0 {
                    notify(t, crate::host::last_line(&out.text));
                }
            }
            tasks.update(&id, |t| {
                t.ended = crate::id::now();
                if t.status != "stopped" {
                    t.status = "exited".into();
                }
            });
            tasks.live.lock().unwrap().remove(&id);
        });
        Ok(info)
    }

    /// Queues `code` in the live REPL of task `id`; returns its place in the queue.
    pub fn add(&self, id: &str, code: &str) -> Result<usize, String> {
        let t = self.get(id).ok_or(format!("no task {id}"))?;
        if t.kind != REPL {
            return Err(format!("task {id} is a process, not a REPL"));
        }
        let live = self.live.lock().unwrap();
        let Some(live) = live.get(id).filter(|_| t.alive()) else {
            return Err(format!(
                "task {id} is {}; start a new one with task_run",
                t.status
            ));
        };
        let place = self.update(id, |t| t.queue += 1).map_or(1, |t| t.queue);
        live.tx
            .send(code.to_string())
            .map_err(|_| format!("task {id} is closed"))?;
        Ok(place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuse_py::{Args, Fail, Host, MontyObject};

    struct Boom;
    impl Host for Boom {
        fn functions(&self) -> &'static [&'static str] {
            &["boom"]
        }
        fn call(&self, _: &str, _: &Args, _: &Ctx) -> Result<MontyObject, Fail> {
            panic!("boom")
        }
        fn cwd(&self) -> std::path::PathBuf {
            std::env::temp_dir()
        }
        fn env(&self) -> Vec<(String, String)> {
            vec![]
        }
        fn module_dirs(&self) -> Vec<std::path::PathBuf> {
            vec![]
        }
    }

    #[test]
    fn a_crashed_repl_task_exits_instead_of_idling() {
        let dir = std::env::temp_dir().join(format!("tuse-repl-crash-{}", std::process::id()));
        let tasks = Tasks::new(dir.clone());
        let (tx, rx) = channel::<String>();
        let notify: Notify = Arc::new(move |t, _| tx.send(t.status).unwrap());
        let make = Box::new(|| Python::new(Arc::new(Boom), 1000));
        let info = tasks.run_repl("boom()", None, &dir, make, notify).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "exited");
        for _ in 0..100 {
            if tasks.live.lock().unwrap().is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let t = tasks.get(&info.id).unwrap();
        assert_eq!((t.status.as_str(), t.exit), ("exited", Some(1)));
        assert!(tasks.add(&info.id, "1").is_err());
        std::fs::remove_dir_all(dir).ok();
    }
}
