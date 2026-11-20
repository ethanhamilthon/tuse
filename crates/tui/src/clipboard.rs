use arboard::Clipboard;
use std::path::{Path, PathBuf};

pub enum Pasted {
    Image(PathBuf),
    Text(String),
    Nothing,
}

#[cfg(not(test))]
pub fn write(text: &str) -> Result<(), String> {
    // On X11 the clipboard owner must stay alive to serve paste requests.
    thread_local! {static OWNER: std::cell::RefCell<Option<Clipboard>> = const { std::cell::RefCell::new(None) };}
    OWNER.with(|cell| {
        let mut owner = cell.borrow_mut();
        if owner.is_none() {
            *owner = Some(Clipboard::new().map_err(|e| e.to_string())?);
        }
        owner
            .as_mut()
            .unwrap()
            .set_text(text)
            .map_err(|e| e.to_string())
    })
}
#[cfg(test)]
pub fn write(text: &str) -> Result<(), String> {
    TEST_TEXT.with(|t| *t.borrow_mut() = text.into());
    Ok(())
}
#[cfg(test)]
thread_local! {pub static TEST_TEXT: std::cell::RefCell<String> = const {std::cell::RefCell::new(String::new())};}

/// Reads the system clipboard: an image is saved as a PNG under `dir`.
pub fn read(dir: &Path) -> Result<Pasted, String> {
    let mut cb = Clipboard::new().map_err(|e| e.to_string())?;
    if let Ok(img) = cb.get_image() {
        let n = std::fs::read_dir(dir).map_or(0, |d| d.count()) + 1;
        let path = dir.join(format!("image-{n}.png"));
        write_png(&path, img.width, img.height, &img.bytes)?;
        return Ok(Pasted::Image(path));
    }
    Ok(match cb.get_text() {
        Ok(t) if !t.is_empty() => Pasted::Text(t),
        _ => Pasted::Nothing,
    })
}

fn write_png(path: &Path, w: usize, h: usize, rgba: &[u8]) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(rgba))
        .map_err(|e| e.to_string())
}
