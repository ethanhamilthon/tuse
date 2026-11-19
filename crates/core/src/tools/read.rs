use super::Output;
use base64::Engine;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tuse_config::Config;

const DEFAULT_LIMIT: usize = 2000;
/// Characters of one line shown; the rest is cut.
const LINE_CHARS: usize = 2000;

pub fn run(path: &Path, args: &Value, config: &Config) -> Output {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Output::err(format!("{}: {e}", path.display())),
    };
    if meta.is_dir() {
        return Output::err(format!(
            "{} is a directory; list it with the python tool (os.listdir)",
            path.display()
        ));
    }
    if let Some(mime) = crate::input::image_mime(path) {
        return image(
            path,
            mime,
            meta.len() as usize,
            config.settings.max_image_bytes,
        );
    }
    let offset = args["offset"].as_u64().unwrap_or(1).max(1) as usize;
    let limit = args["limit"]
        .as_u64()
        .map_or(DEFAULT_LIMIT, |n| n.max(1) as usize);
    match text(path, offset, limit, config.settings.max_output) {
        Ok(t) => Output::ok(t),
        Err(e) => Output::err(format!("{}: {e}", path.display())),
    }
}

fn image(path: &Path, mime: &str, size: usize, max: usize) -> Output {
    if size > max {
        return Output::err(format!(
            "{} is {size} bytes, over the image limit of {max} bytes (settings.max_image_bytes)",
            path.display()
        ));
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Output {
                text: format!("[image: {}, {} KB]", path.display(), size.div_ceil(1024)),
                ok: true,
                images: vec![format!("data:{mime};base64,{data}")],
            }
        }
        Err(e) => Output::err(format!("{}: {e}", path.display())),
    }
}

/// Numbered lines `offset..offset + limit`, within `budget` bytes.
fn text(path: &Path, offset: usize, limit: usize, budget: usize) -> std::io::Result<String> {
    let mut reader = BufReader::new(std::fs::File::open(path)?);
    if reader.fill_buf()?.iter().take(8192).any(|&b| b == 0) {
        let size = std::fs::metadata(path)?.len();
        return Ok(format!("[binary file, {size} bytes]"));
    }
    let mut out = String::new();
    let (mut n, mut last, mut buf) = (0usize, 0usize, Vec::new());
    let mut full = false;
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        n += 1;
        if n < offset || full {
            continue;
        }
        let line = String::from_utf8_lossy(&buf);
        let line = line.trim_end_matches(['\n', '\r']);
        let line = match line.char_indices().nth(LINE_CHARS) {
            Some((i, _)) => format!("{}… [line cut]", &line[..i]),
            None => line.to_string(),
        };
        let row = format!("{n}\t{line}\n");
        if last >= offset && (n - offset >= limit || out.len() + row.len() > budget) {
            full = true;
            continue;
        }
        out.push_str(&row);
        last = n;
    }
    if n == 0 {
        return Ok("[empty file]".into());
    }
    if last < offset {
        return Ok(format!(
            "[offset {offset} is past the end of the file ({n} lines)]"
        ));
    }
    if last < n {
        out.push_str(&format!(
            "[lines {offset}-{last} of {n}; continue with offset={}]",
            last + 1
        ));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_through_a_file_and_flags_binary() {
        let dir = std::env::temp_dir().join(format!("tuse-read-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("t.txt");
        std::fs::write(&f, "a\nb\nc\nd\n").unwrap();
        let t = text(&f, 2, 2, 1000).unwrap();
        assert_eq!(t, "2\tb\n3\tc\n[lines 2-3 of 4; continue with offset=4]");
        assert!(text(&f, 9, 2, 1000).unwrap().contains("past the end"));
        std::fs::write(&f, "").unwrap();
        assert_eq!(text(&f, 1, 2, 1000).unwrap(), "[empty file]");
        std::fs::write(&f, b"ab\0cd").unwrap();
        assert!(text(&f, 1, 2, 1000).unwrap().starts_with("[binary file"));
        std::fs::remove_dir_all(dir).ok();
    }
}
