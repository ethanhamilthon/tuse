use crate::record::UserInput;
use base64::Engine;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Text files longer than this are cut when attached.
pub const FILE_LIMIT: usize = 1024;

/// What a message refers to besides its text.
#[derive(Debug, Default)]
pub struct Refs {
    /// Prompts as `(name, body)`; repeats of a name are dropped.
    pub prompts: Vec<(String, String)>,
    /// `[Image #N]` placeholders as `(N, path)`.
    pub images: Vec<(usize, PathBuf)>,
    /// Files; images among them are attached like pictures.
    pub files: Vec<PathBuf>,
    /// Fail on unreadable files instead of skipping them.
    pub strict: bool,
    /// Relative paths are read from here; they are shown as given.
    pub cwd: PathBuf,
}

/// Builds a message: `text` stays as typed, the prompts, images and files go
/// to `context` as XML blocks, and the pictures to `images` as `data:` URLs.
pub fn build(text: &str, refs: Refs) -> Result<UserInput, String> {
    let mut input = UserInput::text(text);
    let mut blocks: Vec<String> = vec![];
    let mut seen: Vec<&str> = vec![];
    for (name, body) in &refs.prompts {
        if !seen.contains(&name.as_str()) {
            seen.push(name);
            blocks.push(format!(
                "<prompt name=\"{}\">{}</prompt>",
                esc(name),
                body.trim_end()
            ));
        }
    }
    let mut next_id = refs.images.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;
    let image = |input: &mut UserInput, id: usize, path: &Path, bytes: &[u8]| {
        let mime = image_mime(path).unwrap_or("image/png");
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        input.images.push(format!("data:{mime};base64,{data}"));
        format!(
            "<image id=\"{id}\" path=\"{}\"/>",
            esc(&path.display().to_string())
        )
    };
    for (id, path) in &refs.images {
        match std::fs::read(refs.cwd.join(path)) {
            Ok(bytes) => blocks.push(image(&mut input, *id, path, &bytes)),
            Err(e) if refs.strict => return Err(format!("{}: {e}", path.display())),
            Err(_) => {}
        }
    }
    let mut done: Vec<&PathBuf> = vec![];
    for path in &refs.files {
        if done.contains(&path) {
            continue;
        }
        done.push(path);
        let shown = esc(&path.display().to_string());
        if image_mime(path).is_some() {
            match std::fs::read(refs.cwd.join(path)) {
                Ok(bytes) => {
                    blocks.push(image(&mut input, next_id, path, &bytes));
                    next_id += 1;
                }
                Err(e) if refs.strict => return Err(format!("{}: {e}", path.display())),
                Err(_) => {}
            }
            continue;
        }
        let full = refs.cwd.join(path);
        if full.is_dir() {
            match list_dir(&full) {
                Ok(list) => blocks.push(format!("<dir path=\"{shown}\">{list}</dir>")),
                Err(e) if refs.strict => return Err(format!("{}: {e}", path.display())),
                Err(_) => {}
            }
            continue;
        }
        match read_head(&full) {
            Ok((head, size)) => blocks.push(match head {
                None => format!("<file path=\"{shown}\" binary=\"true\" size=\"{size}\"/>"),
                Some(t) if size as usize > FILE_LIMIT => {
                    format!("<file path=\"{shown}\" truncated=\"true\" size=\"{size}\">{t}</file>")
                }
                Some(t) => format!("<file path=\"{shown}\">{t}</file>"),
            }),
            Err(e) if refs.strict => return Err(format!("{}: {e}", path.display())),
            Err(_) => {}
        }
    }
    input.context = blocks.join("\n");
    Ok(input)
}

/// A folder's entries, sorted, folders with a trailing `/`, cut at `FILE_LIMIT` bytes.
fn list_dir(path: &Path) -> std::io::Result<String> {
    let mut names: Vec<String> = std::fs::read_dir(path)?
        .flatten()
        .map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                format!("{n}/")
            } else {
                n
            }
        })
        .collect();
    names.sort();
    let mut out = String::new();
    for n in names {
        if out.len() + n.len() > FILE_LIMIT {
            out.push_str("\n…");
            break;
        }
        out.push('\n');
        out.push_str(&n);
    }
    out.push('\n');
    Ok(esc(&out))
}

/// The first `FILE_LIMIT` bytes as text (`None` for a binary file) and the file size.
fn read_head(path: &Path) -> std::io::Result<(Option<String>, u64)> {
    let file = std::fs::File::open(path)?;
    let meta = file.metadata()?;
    if meta.is_dir() {
        return Err(std::io::Error::other("is a directory"));
    }
    let mut buf = vec![];
    file.take(FILE_LIMIT as u64).read_to_end(&mut buf)?;
    let size = meta.len();
    let cut = size as usize > buf.len();
    let text = match std::str::from_utf8(&buf) {
        Ok(s) => Some(s.to_string()),
        // An unfinished character at the cut is not an error.
        Err(e) if cut && e.error_len().is_none() => {
            Some(String::from_utf8_lossy(&buf[..e.valid_up_to()]).into_owned())
        }
        Err(_) => None,
    };
    Ok((text.filter(|t| !t.contains('\0')), size))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

pub(crate) fn image_mime(p: &Path) -> Option<&'static str> {
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    })
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
