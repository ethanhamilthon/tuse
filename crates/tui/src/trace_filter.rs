//! The `/trace` query: `+type` keeps only those types (several are OR), `-type` drops one
//! (from the kept ones, or from all when there is no `+`), other words search the text.
pub const TYPES: &[&str] = &[
    "system",
    "user",
    "agent",
    "thinking",
    "lua",
    "tool",
    "output",
    "notification",
    "notice",
    "error",
    "hook",
];

#[derive(Debug, Default, PartialEq)]
pub struct Query {
    pub keep: Vec<String>,
    pub drop: Vec<String>,
    /// The words without a prefix, joined by spaces.
    pub text: String,
    /// An unknown type was named: nothing matches.
    pub bad: bool,
}

pub fn parse(q: &str) -> Query {
    let mut out = Query::default();
    let mut words = vec![];
    for word in q.split_whitespace() {
        if !word.starts_with(['+', '-']) {
            words.push(word);
            continue;
        }
        // `+lua+agent` and `+lua-agent` are several tokens in one word.
        let mut rest = word;
        while let Some(sign) = rest.chars().next().filter(|c| matches!(c, '+' | '-')) {
            let body = &rest[1..];
            let end = body.find(['+', '-']).unwrap_or(body.len());
            let name = &body[..end];
            rest = &body[end..];
            if name.is_empty() {
                continue;
            }
            if !TYPES.contains(&name) {
                out.bad = true;
            }
            let list = if sign == '+' {
                &mut out.keep
            } else {
                &mut out.drop
            };
            list.push(name.to_string());
        }
    }
    out.text = words.join(" ");
    out
}

impl Query {
    pub fn allows(&self, role: &str) -> bool {
        !self.bad
            && (self.keep.is_empty() || self.keep.iter().any(|k| k == role))
            && !self.drop.iter().any(|d| d == role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_types_and_words() {
        let allowed = |q: &str| -> Vec<&str> {
            let q = parse(q);
            TYPES.iter().copied().filter(|t| q.allows(t)).collect()
        };
        assert_eq!(allowed("+lua+agent"), ["agent", "lua"]);
        assert_eq!(allowed("+lua read"), ["lua"]);
        assert_eq!(parse("+lua read  me").text, "read me");
        assert_eq!(allowed("-hook").len(), TYPES.len() - 1);
        assert!(allowed("+agent-agent").is_empty());
        assert!(allowed("+hooks").is_empty() && parse("+hooks").bad);
        assert_eq!(allowed("+").len(), TYPES.len());
        assert_eq!(parse("+ word").text, "word");
    }
}
