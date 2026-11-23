use crate::Host;
use crate::job::{Done, Job, Out, Stop};
use crate::sched::{self, Sched};
use mlua::{Lua, MultiValue};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

type Cmd = Box<dyn FnOnce(&Lua) + Send>;

pub struct Vm {
    tx: Sender<Cmd>,
}

impl Vm {
    pub fn new(host: Arc<dyn Host>, setup: impl FnOnce(&Lua) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        std::thread::Builder::new()
            .name("tuse-lua".into())
            .spawn(move || {
                let lua = Lua::new();
                lua.set_app_data(Sched::default());
                lua.set_app_data(host);
                if let Err(e) = crate::api::install(&lua) {
                    eprintln!("tuse: lua: {}", crate::message(&e));
                }
                setup(&lua);
                run(&lua, &rx);
            })
            .expect("spawn the lua thread");
        Self { tx }
    }

    pub fn call<R: Send + 'static>(&self, f: impl FnOnce(&Lua) -> R + Send + 'static) -> R {
        let (tx, rx) = mpsc::channel();
        let sent = self.tx.send(Box::new(move |lua: &Lua| {
            let _ = tx.send(f(lua));
        }));
        assert!(sent.is_ok(), "the lua thread is gone");
        rx.recv().expect("the lua thread is gone")
    }

    pub fn post(&self, f: impl FnOnce(&Lua) + Send + 'static) {
        let _ = self.tx.send(Box::new(f));
    }

    pub fn run(&self, code: &str, name: &str, stop: Stop) -> Done {
        let (tx, rx) = mpsc::channel();
        let (code, name) = (code.to_string(), format!("={name}"));
        self.post(move |lua| {
            let tx2 = tx.clone();
            let f = match compile(lua, &code, &name) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(fail(crate::message(&e)));
                    return;
                }
            };
            let job = Job::new(
                lua,
                f,
                MultiValue::new(),
                stop,
                Out::Buf(String::new()),
                move |_, d| {
                    let _ = tx2.send(d);
                },
            );
            match job {
                Ok(j) => j.start(lua),
                Err(e) => {
                    let _ = tx.send(fail(crate::message(&e)));
                }
            }
        });
        rx.recv()
            .unwrap_or_else(|_| fail("the lua thread is gone".into()))
    }
}

fn fail(text: String) -> Done {
    Done {
        ok: false,
        text,
        stop: None,
        diffs: vec![],
        images: vec![],
    }
}

fn compile(lua: &Lua, code: &str, name: &str) -> mlua::Result<mlua::Function> {
    lua.load(format!("return {code}"))
        .set_name(name)
        .into_function()
        .or_else(|_| lua.load(code).set_name(name).into_function())
}

fn run(lua: &Lua, rx: &Receiver<Cmd>) {
    loop {
        let idle = lua.app_data_ref::<Sched>().is_none_or(|s| s.is_empty());
        let cmd = if idle {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            }
        } else {
            match rx.recv_timeout(Duration::from_millis(5)) {
                Ok(c) => Some(c),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        };
        if let Some(c) = cmd {
            c(lua);
            while let Ok(c) = rx.try_recv() {
                c(lua);
            }
        }
        while sched::tick(lua) {}
    }
}
