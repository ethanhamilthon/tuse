use crate::proto::{Request, Response};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

pub fn call(path: &Path, req: &Request) -> Result<Response, String> {
    let mut stream = UnixStream::connect(path).map_err(|e| format!("session not running ({e})"))?;
    let mut text = serde_json::to_string(req).map_err(|e| e.to_string())?;
    text.push('\n');
    stream
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&line).map_err(|e| format!("bad response: {e}"))
}
