pub struct Args {
    pub positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    pub fn parse(raw: &[String], valued: &[&str]) -> Result<Args, String> {
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
                let value = if valued.contains(&name.as_str()) {
                    match inline {
                        Some(v) => Some(v),
                        None => Some(
                            it.next()
                                .filter(|v| !v.starts_with('-'))
                                .cloned()
                                .ok_or(format!("{name} needs a value"))?,
                        ),
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
