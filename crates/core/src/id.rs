use time::OffsetDateTime;
use time::macros::format_description;

pub fn new_session_id() -> String {
    let now = OffsetDateTime::now_utc();
    let f = format_description!("[year][month][day]T[hour][minute][second].[subsecond digits:9]Z");
    let mut rnd = [0u8; 8];
    let _ = getrandom::fill(&mut rnd);
    let hex: String = rnd.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{hex}", now.format(f).unwrap_or_default())
}

pub fn now() -> String {
    let f = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    OffsetDateTime::now_utc().format(f).unwrap_or_default()
}

pub fn elapsed_since(started: &str, ended: &str) -> Option<std::time::Duration> {
    let f = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    let parse = |s: &str| {
        time::PrimitiveDateTime::parse(s, f)
            .ok()
            .map(|t| t.assume_utc())
    };
    let a = parse(started)?;
    let b = if ended.is_empty() {
        OffsetDateTime::now_utc()
    } else {
        parse(ended)?
    };
    (b - a).try_into().ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats() {
        let id = super::new_session_id();
        assert_eq!(
            id.len(),
            "20261119T004924.125476000Z-7136dd19e3fc184d".len(),
            "{id}"
        );
        assert!(id.as_bytes()[8] == b'T' && id.contains("Z-"), "{id}");
        let now = super::now();
        assert!(
            now.len() == 20 && now.ends_with('Z') && now.as_bytes()[10] == b'T',
            "{now}"
        );
    }
}
