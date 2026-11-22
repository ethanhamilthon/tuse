use crate::record::Record;
use crate::store::{Meta, SessionDir, read_meta};
use std::io::BufRead;

/// Moves sessions of older versions (`<id>/session.jsonl` and `meta.json`) into the
/// database once: the first open after an update does it, then the files are renamed.
pub(crate) fn sessions() {
    let Ok(dirs) = std::fs::read_dir(crate::store::tuse_home()) else {
        return;
    };
    for dir in dirs.flatten().map(|e| e.path()) {
        let (log, meta) = (dir.join("session.jsonl"), dir.join("meta.json"));
        let Some(m) = std::fs::read_to_string(&meta)
            .ok()
            .and_then(|t| serde_json::from_str::<Meta>(&t).ok())
        else {
            continue;
        };
        if read_meta(&m.id).is_none() && one(&m, &log).is_err() {
            continue;
        }
        let _ = std::fs::rename(&log, dir.join("session.jsonl.old"));
        let _ = std::fs::rename(&meta, dir.join("meta.json.old"));
    }
}

fn one(m: &Meta, log: &std::path::Path) -> Result<(), String> {
    let mut d = SessionDir::open(&m.id, false)?;
    if let Ok(f) = std::fs::File::open(log) {
        for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
            if let Ok(r) = serde_json::from_str::<Record>(&line) {
                d.append(&r);
            }
        }
    }
    d.write_meta(m);
    Ok(())
}
