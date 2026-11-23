/// `5200` -> `5.2k`, `1_000_000` -> `1m`: one compact form for both sides of `used/max`.
pub fn short(n: u64) -> String {
    let (v, unit) = match n {
        n if n >= 1_000_000 => (n as f64 / 1e6, "m"),
        n if n >= 1000 => (n as f64 / 1e3, "k"),
        n => return n.to_string(),
    };
    let s = format!("{v:.1}");
    format!("{}{unit}", s.strip_suffix(".0").unwrap_or(&s))
}

/// The `used/max` context indicator; `?` while the maximum is unknown.
pub fn usage(used: u64, max: u64) -> String {
    let max = if max > 0 { short(max) } else { "?".into() };
    format!("{}/{max}", short(used))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_format() {
        assert_eq!(short(5200), "5.2k");
        assert_eq!(short(1_000_000), "1m");
        assert_eq!(short(200_000), "200k");
        assert_eq!(short(999), "999");
        assert_eq!(usage(5200, 1_000_000), "5.2k/1m");
        assert_eq!(usage(5200, 0), "5.2k/?");
    }
}
