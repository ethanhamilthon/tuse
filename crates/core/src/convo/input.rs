use crate::convo::record::UserInput;
use base64::Engine;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const FILE_LIMIT: usize = 1024;

#[derive(Debug, Default)]
pub struct Refs {
    pub prompts: Vec<(String, String)>,
    pub images: Vec<(usize, PathBuf)>,
    pub files: Vec<PathBuf>,
    pub strict: bool,
    pub cwd: PathBuf,
}

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
        let mime = crate::text::image_mime(path).unwrap_or("image/png");
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
        if crate::text::image_mime(path).is_some() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tuse-input-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn prompts_are_deduplicated_and_text_stays_clean() {
        let refs = Refs {
            prompts: vec![
                ("a".into(), "one\nline  \n".into()),
                ("b\"".into(), "two".into()),
                ("a".into(), "again".into()),
            ],
            ..Refs::default()
        };
        let i = build("#a #b #a", refs).unwrap();
        assert_eq!(i.text, "#a #b #a");
        assert_eq!(
            i.context,
            "<prompt name=\"a\">one\nline</prompt>\n<prompt name=\"b&quot;\">two</prompt>"
        );
    }

    #[test]
    fn images_keep_ids_and_get_the_next_free_one_from_files() {
        let a = tmp("a.png", b"PNG");
        let b = tmp("b.JPG", b"JPG");
        let refs = Refs {
            images: vec![(3, a.clone())],
            files: vec![b.clone(), b.clone()],
            ..Refs::default()
        };
        let i = build("[Image #3]", refs).unwrap();
        assert_eq!(i.images.len(), 2);
        assert!(i.images[0].starts_with("data:image/png;base64,"));
        assert!(i.images[1].starts_with("data:image/jpeg;base64,"));
        assert!(
            i.context
                .contains(&format!("<image id=\"3\" path=\"{}\"/>", a.display()))
        );
        assert!(
            i.context
                .contains(&format!("<image id=\"4\" path=\"{}\"/>", b.display()))
        );
    }

    #[test]
    fn text_files_are_cut_on_a_utf8_boundary() {
        let small = tmp("s.txt", b"hi\n");
        let exact = tmp("e.txt", "x".repeat(FILE_LIMIT).as_bytes());
        let big = tmp(
            "b.txt",
            format!("{}é tail", "x".repeat(FILE_LIMIT - 1)).as_bytes(),
        );
        let refs = |p: &PathBuf| Refs {
            files: vec![p.clone()],
            ..Refs::default()
        };
        let c = build("", refs(&small)).unwrap().context;
        assert_eq!(c, format!("<file path=\"{}\">hi\n</file>", small.display()));
        assert!(
            !build("", refs(&exact))
                .unwrap()
                .context
                .contains("truncated")
        );
        let c = build("", refs(&big)).unwrap().context;
        assert!(c.contains("truncated=\"true\" size=\"1030\""));
        assert!(c.ends_with(&format!("{}</file>", "x".repeat(FILE_LIMIT - 1))));
    }

    #[test]
    fn binary_files_have_no_content_and_missing_ones_follow_strict() {
        let nul = tmp("n.dat", b"ab\0cd");
        let bad = tmp("bad.txt", &[0xff, 0xfe, b'a']);
        for p in [&nul, &bad] {
            let c = build(
                "",
                Refs {
                    files: vec![p.clone()],
                    ..Refs::default()
                },
            )
            .unwrap()
            .context;
            assert!(c.contains("binary=\"true\"") && c.ends_with("/>"), "{c}");
        }
        let gone = PathBuf::from("/definitely/missing.txt");
        let lax = Refs {
            files: vec![gone.clone()],
            ..Refs::default()
        };
        assert_eq!(build("x", lax).unwrap().context, "");
        let dir = tmp("listed", b"").with_extension("d");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let refs = Refs {
            files: vec![dir.clone()],
            ..Refs::default()
        };
        let shown = dir.display();
        assert_eq!(
            build("x", refs).unwrap().context,
            format!("<dir path=\"{shown}\">\na.txt\nsub/\n</dir>")
        );
        let strict = Refs {
            files: vec![gone],
            strict: true,
            ..Refs::default()
        };
        assert!(build("x", strict).is_err());
    }
}
