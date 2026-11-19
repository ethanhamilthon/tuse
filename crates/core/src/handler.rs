use crate::event::Event;
use crate::record::UserInput;
use crate::session::Session;
use crate::tasks;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tuse_socket::{Request, Response};

impl Session {
    /// Serves one socket request (`tuse steer`, `tuse ask`, `tuse task ...`).
    pub(crate) fn handle(self: Arc<Self>, req: Request) -> Response {
        match req {
            Request::Steer { text, files } => match crate::input::with_files(&text, &files) {
                Ok(input) => {
                    let busy = self.busy();
                    self.submit(input);
                    Response::ok(json!({"queued": busy}))
                }
                Err(e) => Response::err(e),
            },
            Request::Ask {
                question,
                options,
                is_async,
            } => self.handle_ask(question, options, is_async),
            Request::TaskRun { command, name, cwd } => {
                let cwd = cwd.map(PathBuf::from).unwrap_or_else(|| self.cwd.clone());
                let weak = Arc::downgrade(&self);
                let on_exit = Box::new(move |t: tasks::TaskInfo| {
                    if let Some(s) = weak.upgrade() {
                        s.emit(Event::Tasks);
                        let code = t
                            .code
                            .map(|c| c.to_string())
                            .unwrap_or("none (killed)".into());
                        s.submit(UserInput::text(format!(
                            "<notification>Task {} ({}) exited with code {code}. Output: `tuse task get {}`.</notification>",
                            t.id, t.name, t.id
                        )));
                    }
                });
                let r = self.tasks.run(&command, name, &cwd, &self.env(), on_exit);
                self.emit(Event::Tasks);
                match r {
                    Ok(t) => Response::ok(json!(t)),
                    Err(e) => Response::err(e),
                }
            }
            Request::TaskStop { id } => {
                let r = self.tasks.stop(&id);
                self.emit(Event::Tasks);
                match r {
                    Ok(t) => Response::ok(json!(t)),
                    Err(e) => Response::err(e),
                }
            }
            Request::TaskGet { id, lines } => match self.tasks.get(&id) {
                Some(t) => {
                    let output = tasks::tail(&t.log, lines.unwrap_or(50));
                    Response::ok(json!({"task": t, "output": output}))
                }
                None => Response::err(format!("no task {id}")),
            },
            Request::Print { text, id } => match self.print(text, id) {
                Ok(id) => Response::ok(json!({ "id": id })),
                Err(e) => Response::err(e),
            },
            Request::TaskList => Response::ok(json!(self.tasks.list())),
            Request::Status => {
                let (model, effort) = self.model();
                Response::ok(json!({
                    "id": self.id, "cwd": self.cwd, "model": model, "effort": effort,
                    "busy": self.busy(), "headless": self.headless, "title": self.title(),
                }))
            }
        }
    }

    fn handle_ask(
        self: Arc<Self>,
        question: String,
        options: Vec<String>,
        is_async: bool,
    ) -> Response {
        let (id, rx) = match self.ask(&question, options) {
            Ok(v) => v,
            Err(e) => return Response::err(e),
        };
        if !is_async {
            return match rx.recv() {
                Ok(a) => Response::ok(json!({"id": id, "answer": a})),
                Err(_) => Response::err("dismissed"),
            };
        }
        let s = self.clone();
        let ask_id = id.clone();
        std::thread::spawn(move || {
            let answer = rx.recv().unwrap_or_else(|_| "(dismissed)".into());
            s.submit(UserInput::text(format!(
                "<notification>Answer to ask {ask_id} \"{question}\": {answer}</notification>"
            )));
        });
        Response::ok(json!({"id": id}))
    }
}
