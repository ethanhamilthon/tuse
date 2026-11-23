use super::fail;
use base64::Engine;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub const DEFAULT_LIMIT: usize = 2000;
/// Characters of one line shown; the rest is cut.
const LINE_CHARS: usize = 2000;

/// What `read` gives back: the text, and a `data:` URL for an image.
pub struct Read {
    pub text: String,
    pub image: Option<String>,
}

pub fn run(
    path: &Path,
    lines: bool,
    offset: usize,
    limit: usize,
    max_image: usize,
) -> Result<Read, String> {
    let io = |e: std::io::Error| super::io_fail(path, &e);
    let meta = std::fs::metadata(path).map_err(io)?;
    if meta.is_dir() {
        return Err(fail(format!(
            "{} is a directory; list it with sh(\"ls\")",
            path.display()
        )));
    }
    if let Some(mime) = crate::input::image_mime(path) {
        return image(path, mime, meta.len() as usize, max_image);
    }
    let text = if lines {
        text(path, offset, limit).map_err(io)?
    } else {
        raw(path).map_err(io)?
    };
    Ok(Read { text, image: None })
}

fn image(path: &Path, mime: &str, size: usize, max: usize) -> Result<Read, String> {
    if size > max {
        return Err(fail(format!(
            "{} is {size} bytes, over the image limit of {max} bytes (settings.max_image_bytes)",
            path.display()
        )));
    }
    let bytes = std::fs::read(path).map_err(|e| super::io_fail(path, &e))?;
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Read {
        text: format!("[image: {}, {} KB]", path.display(), size.div_ceil(1024)),
        image: Some(format!("data:{mime};base64,{data}")),
    })
}

/// Raw file text, keeping binary detection intact.
fn raw(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    if bytes.iter().take(8192).any(|&b| b == 0) {
        return Ok(format!("[binary file, {} bytes]", bytes.len()));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Numbered lines `offset..offset + limit`.
fn text(path: &Path, offset: usize, limit: usize) -> std::io::Result<String> {
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
        if last >= offset && n - offset >= limit {
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
    fn reads_raw_by_default_pages_with_lines_and_flags_binary() {
        let dir = std::env::temp_dir().join(format!("tuse-read-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("t.txt");
        std::fs::write(&f, "a\nb\nc\nd\n").unwrap();
        assert_eq!(run(&f, false, 1, 2, 1024).unwrap().text, "a\nb\nc\nd\n");
        let t = run(&f, true, 2, 2, 1024).unwrap().text;
        assert_eq!(t, "2\tb\n3\tc\n[lines 2-3 of 4; continue with offset=4]");
        assert!(text(&f, 9, 2).unwrap().contains("past the end"));
        std::fs::write(&f, "").unwrap();
        assert_eq!(run(&f, false, 1, 2, 1024).unwrap().text, "");
        assert_eq!(text(&f, 1, 2).unwrap(), "[empty file]");
        std::fs::write(&f, b"ab\0cd").unwrap();
        assert!(
            run(&f, false, 1, 2, 1024)
                .unwrap()
                .text
                .starts_with("[binary file")
        );
        assert!(text(&f, 1, 2).unwrap().starts_with("[binary file"));
        std::fs::remove_dir_all(dir).ok();
    }
}
