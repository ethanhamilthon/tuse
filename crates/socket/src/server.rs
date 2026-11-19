use crate::proto::{Request, Response};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub type Handler = Arc<dyn Fn(Request) -> Response + Send + Sync>;

/// Listens on a Unix socket, one thread per connection, one JSON line per request.
/// Dropping it stops accepting and removes the socket file.
pub struct Server {
    path: PathBuf,
    stop: Arc<AtomicBool>,
}

impl Server {
    pub fn start(path: &Path, handler: Handler) -> std::io::Result<Server> {
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let h = handler.clone();
                        std::thread::spawn(move || serve(stream, h));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(50)),
                }
            }
        });
        Ok(Server {
            path: path.to_path_buf(),
            stop,
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&self.path);
    }
}

fn serve(stream: UnixStream, handler: Handler) {
    let _ = stream.set_nonblocking(false);
    let Ok(mut out) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { return };
        if line.trim().is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handler(req),
            Err(e) => Response::err(format!("bad request: {e}")),
        };
        let mut text = serde_json::to_string(&resp).unwrap_or_default();
        text.push('\n');
        if out.write_all(text.as_bytes()).is_err() {
            return;
        }
    }
}
