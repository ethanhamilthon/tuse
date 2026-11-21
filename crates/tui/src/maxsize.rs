use crate::app::App;

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

/// `200k`, `1m`, `1.5m` or a plain number; zero and anything else is rejected.
pub fn parse(text: &str) -> Option<u64> {
    let t = text.trim().to_ascii_lowercase();
    let (num, mult) = match t.chars().last()? {
        'k' => (&t[..t.len() - 1], 1e3),
        'm' => (&t[..t.len() - 1], 1e6),
        _ => (t.as_str(), 1.0),
    };
    if !num.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let n = (num.parse::<f64>().ok()? * mult).round();
    (1.0..1e15).contains(&n).then_some(n as u64)
}

/// The `used/max` context indicator; `?` while the maximum is unknown.
pub fn usage(used: u64, max: u64) -> String {
    let max = if max > 0 { short(max) } else { "?".into() };
    format!("{}/{max}", short(used))
}

impl App {
    /// `/maxsize 1m` sets the maximum; `/maxsize off` goes back to the model's window.
    pub(crate) fn set_maxsize(&mut self, arg: &str) {
        match arg.trim() {
            "" => {
                let now = self.max_size.map_or("not set".to_string(), short);
                self.flash(format!(
                    "Max size: {now}. Use /maxsize 200k, /maxsize 1m or /maxsize off."
                ));
            }
            "off" => {
                self.max_size = None;
                self.flash("Max size follows the model.");
            }
            text => match parse(text) {
                Some(n) => {
                    self.max_size = Some(n);
                    self.flash(format!("Max size: {}.", short(n)));
                }
                None => self.error(format!("Bad size {text:?}. Use 200k, 1m or a number.")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_format_and_parse() {
        assert_eq!(short(5200), "5.2k");
        assert_eq!(short(1_000_000), "1m");
        assert_eq!(short(200_000), "200k");
        assert_eq!(short(999), "999");
        assert_eq!(usage(5200, 1_000_000), "5.2k/1m");
        assert_eq!(usage(5200, 0), "5.2k/?");
        assert_eq!(parse("200k"), Some(200_000));
        assert_eq!(parse(" 1M "), Some(1_000_000));
        assert_eq!(parse("1.5m"), Some(1_500_000));
        assert_eq!(parse("32000"), Some(32_000));
        for bad in ["", "0", "k", "abc", "-5k", "1x", "1.2.3k", "1 k"] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_command_sets_the_max_and_the_status_shows_it() {
        let mut app = crate::app::test_app("maxsize");
        app.run_command("maxsize 1m");
        assert_eq!(app.max_size, Some(1_000_000));
        assert!(
            crate::status::model(&app).contains("/1m"),
            "{}",
            crate::status::model(&app)
        );
        app.run_command("maxsize nope");
        assert_eq!(app.max_size, Some(1_000_000));
        app.run_command("maxsize off");
        assert_eq!(app.max_size, None);
        crate::app::cleanup(app);
    }
}
