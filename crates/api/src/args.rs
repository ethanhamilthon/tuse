/// Incremental decoder for the top-level string fields of tool arguments.
#[derive(Default)]
pub struct ArgStream {
    depth: usize,
    expect_key: bool,
    string: bool,
    target: Target,
    key: String,
    value: String,
    escape: bool,
    hex: Option<(u16, u8)>,
    high: Option<u16>,
}
#[derive(Default, PartialEq)]
enum Target {
    #[default]
    Skip,
    Key,
    Code,
    Title,
}
impl ArgStream {
    pub fn feed(&mut self, chunk: &str) -> (String, Option<String>) {
        let mut code = String::new();
        let mut title = None;
        for c in chunk.chars() {
            if !self.string {
                match c {
                    '{' | '[' => {
                        self.depth = self.depth.saturating_add(1);
                        self.expect_key = self.depth == 1;
                    }
                    '}' | ']' => self.depth = self.depth.saturating_sub(1),
                    ',' if self.depth == 1 => {
                        self.expect_key = true;
                        self.key.clear();
                    }
                    '"' => {
                        self.string = true;
                        self.target = if self.depth != 1 {
                            Target::Skip
                        } else if self.expect_key {
                            Target::Key
                        } else {
                            match self.key.as_str() {
                                "code" => Target::Code,
                                "title" => Target::Title,
                                _ => Target::Skip,
                            }
                        };
                        self.value.clear();
                    }
                    _ => {}
                }
                continue;
            }
            let decoded = if let Some((n, len)) = self.hex.take() {
                let Some(d) = c.to_digit(16) else {
                    self.high = None;
                    continue;
                };
                let n = (n << 4) | d as u16;
                if len < 3 {
                    self.hex = Some((n, len + 1));
                    continue;
                }
                if (0xd800..=0xdbff).contains(&n) {
                    self.high = Some(n);
                    continue;
                }
                if let Some(high) = self.high.take() {
                    char::decode_utf16([high, n]).next().and_then(Result::ok)
                } else {
                    char::from_u32(n as u32)
                }
            } else if self.escape {
                self.escape = false;
                match c {
                    'u' => {
                        self.hex = Some((0, 0));
                        continue;
                    }
                    'n' => Some('\n'),
                    't' => Some('\t'),
                    'r' => Some('\r'),
                    'b' => Some('\x08'),
                    'f' => Some('\x0c'),
                    '"' | '\\' | '/' => Some(c),
                    _ => None,
                }
            } else {
                match c {
                    '\\' => {
                        self.escape = true;
                        continue;
                    }
                    '"' => {
                        self.string = false;
                        self.high = None;
                        match self.target {
                            Target::Key => {
                                self.key = std::mem::take(&mut self.value);
                                self.expect_key = false;
                            }
                            Target::Title => title = Some(std::mem::take(&mut self.value)),
                            _ => {}
                        }
                        continue;
                    }
                    _ => Some(c),
                }
            };
            if let Some(c) = decoded {
                match self.target {
                    Target::Code => code.push(c),
                    Target::Key | Target::Title => self.value.push(c),
                    Target::Skip => {}
                }
            }
        }
        (code, title)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_split_matches_complete_json() {
        for s in [
            r#"{"title":"\u00e9","timeout":3,"code":"print(\"a\\nb\")\n\ud83d\ude03"}"#,
            r#"{"extra":[{"code":"ignore"}],"code":"abc","title":"Run"}"#,
        ] {
            let parsed: serde_json::Value = serde_json::from_str(s).unwrap();
            for i in 0..=s.len() {
                let mut a = ArgStream::default();
                let (mut c, t) = a.feed(&s[..i]);
                let (rest, t2) = a.feed(&s[i..]);
                c.push_str(&rest);
                assert_eq!(c, parsed["code"]);
                assert_eq!(t2.or(t).unwrap(), parsed["title"]);
            }
        }
    }
}
