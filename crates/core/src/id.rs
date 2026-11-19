use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

/// `20261119T004924.125476000Z-7136dd19e3fc184d`: UTC time plus 8 random bytes.
pub fn new_session_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let (y, mo, d, h, mi, s) = civil(now.as_secs());
    let mut rnd = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut rnd);
    }
    let hex: String = rnd.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}.{:09}Z-{hex}",
        now.subsec_nanos()
    )
}

/// RFC 3339 UTC timestamp with seconds precision.
pub fn now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = civil(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn civil(secs: u64) -> (i64, u32, u32, u64, u64, u64) {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    #[test]
    fn civil_dates() {
        assert_eq!(super::civil(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(super::civil(1795049364), (2026, 11, 19, 0, 49, 24));
    }
}
