/// A tiny flag parser: `--name value`, `--flag`, `-s value` and positionals;
/// everything after `--` is positional.
pub struct Args {
    pub positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    /// `valued` lists flags that take a value.
    pub fn parse(raw: &[String], valued: &[&str]) -> Result<Args, String> {
        Self::parse_with(raw, valued, None)
    }

    /// For commands that take free text: only `valued` and `bools` are flags,
    /// so text like `--- menu ---` or `-x` stays text.
    pub fn parse_text(raw: &[String], valued: &[&str], bools: &[&str]) -> Result<Args, String> {
        Self::parse_with(raw, valued, Some(bools))
    }

    fn parse_with(raw: &[String], valued: &[&str], bools: Option<&[&str]>) -> Result<Args, String> {
        let mut a = Args {
            positional: vec![],
            flags: vec![],
        };
        let mut it = raw.iter();
        while let Some(s) = it.next() {
            if s == "--" {
                a.positional.extend(it.by_ref().cloned());
                break;
            }
            if s.starts_with('-') && s.len() > 1 && s != "-" {
                let (name, inline) = match s.split_once('=') {
                    Some((n, v)) => (n.to_string(), Some(v.to_string())),
                    None => (s.clone(), None),
                };
                let is_flag = |n: &str| valued.contains(&n) || bools.is_none_or(|b| b.contains(&n));
                if !is_flag(&name) {
                    a.positional.push(s.clone());
                    continue;
                }
                let value = if valued.contains(&name.as_str()) {
                    match inline {
                        Some(v) => Some(v),
                        None => Some(it.next().cloned().ok_or(format!("{name} needs a value"))?),
                    }
                } else {
                    inline
                };
                a.flags.push((name, value));
            } else {
                a.positional.push(s.clone());
            }
        }
        Ok(a)
    }

    pub fn has(&self, names: &[&str]) -> bool {
        self.flags.iter().any(|(n, _)| names.contains(&n.as_str()))
    }

    pub fn value(&self, names: &[&str]) -> Option<String> {
        self.flags
            .iter()
            .rev()
            .find(|(n, _)| names.contains(&n.as_str()))
            .and_then(|(_, v)| v.clone())
    }

    pub fn values(&self, names: &[&str]) -> Vec<String> {
        self.flags
            .iter()
            .filter(|(n, _)| names.contains(&n.as_str()))
            .filter_map(|(_, v)| v.clone())
            .collect()
    }

    pub fn unknown(&self, known: &[&str]) -> Option<String> {
        self.flags
            .iter()
            .find(|(n, _)| !known.contains(&n.as_str()))
            .map(|(n, _)| n.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn text_commands_keep_dashed_text() {
        let a =
            Args::parse_text(&v(&["--id", "p1", "--- menu ---", "-x"]), &["--id"], &[]).unwrap();
        assert_eq!(a.value(&["--id"]).as_deref(), Some("p1"));
        assert_eq!(a.positional, ["--- menu ---", "-x"]);
    }
}
