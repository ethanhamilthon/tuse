pub struct Args {
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    pub fn parse(raw: &[String], valued: &[&str], plain: &[&str]) -> Result<Args, String> {
        let mut a = Args {
            positional: vec![],
            flags: vec![],
        };
        let is_known_flag = |tok: &str| {
            let name = tok.split_once('=').map_or(tok, |(n, _)| n);
            matches!(name, "-h" | "--help") || valued.contains(&name) || plain.contains(&name)
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
                let value = if valued.contains(&name.as_str()) {
                    match inline {
                        Some(v) => Some(v),
                        None => Some(
                            it.next()
                                .filter(|v| !is_known_flag(v))
                                .cloned()
                                .ok_or(format!("{name} needs a value"))?,
                        ),
                    }
                } else if plain.contains(&name.as_str()) {
                    if inline.is_some() {
                        return Err(format!("{name} does not take a value"));
                    }
                    None
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

    pub fn opt_arg(&self, i: usize) -> Option<&str> {
        self.positional.get(i).map(String::as_str)
    }

    pub fn expect_args(&self, n: usize) -> Result<(), String> {
        self.expect_args_range(n, n)
    }

    pub fn expect_args_range(&self, min: usize, max: usize) -> Result<(), String> {
        let len = self.positional.len();
        if (min..=max).contains(&len) {
            return Ok(());
        }
        Err(match self.positional.get(max) {
            Some(p) => format!("unexpected argument `{p}`"),
            None => "missing argument".into(),
        })
    }

    pub fn value(&self, names: &[&str]) -> Option<String> {
        self.flags
            .iter()
            .rev()
            .find(|(n, _)| names.contains(&n.as_str()))
            .and_then(|(_, v)| v.clone())
    }

    pub fn has(&self, names: &[&str]) -> bool {
        self.flags.iter().any(|(n, _)| names.contains(&n.as_str()))
    }

    pub fn unknown(&self, known: &[&str]) -> Option<String> {
        self.flags
            .iter()
            .find(|(n, _)| !known.contains(&n.as_str()))
            .map(|(n, _)| n.clone())
    }
}
