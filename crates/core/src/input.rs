use crate::record::UserInput;
use base64::Engine;
use std::path::Path;

/// Builds a message from text and files: images become image parts,
/// other files are appended as text.
pub fn with_files(text: &str, files: &[String]) -> Result<UserInput, String> {
    let mut input = UserInput::text(text);
    for f in files {
        let path = Path::new(f);
        let bytes = std::fs::read(path).map_err(|e| format!("{f}: {e}"))?;
        match image_mime(path) {
            Some(mime) => {
                let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
                input.images.push(format!("data:{mime};base64,{data}"));
                input.text.push_str(&format!("\n[Image: {f}]"));
            }
            None => {
                let content = String::from_utf8_lossy(&bytes);
                input.text.push_str(&format!(
                    "\n\n<file path=\"{f}\">\n{}\n</file>",
                    content.trim_end()
                ));
            }
        }
    }
    input.text = input.text.trim_start().to_string();
    Ok(input)
}

/// Builds a message whose text already names the images; they are only attached.
pub fn with_images(text: &str, paths: &[String]) -> Result<UserInput, String> {
    let mut input = UserInput::text(text);
    for f in paths {
        let bytes = std::fs::read(f).map_err(|e| format!("{f}: {e}"))?;
        let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        input.images.push(format!("data:image/png;base64,{data}"));
    }
    Ok(input)
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
