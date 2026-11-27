use serde::Deserialize;
use serde::de::{Deserializer, Error};

macro_rules! tokens {
    ($name:ident { $($field:ident: $default:literal,)* }) => {
        #[derive(Debug, Clone, Deserialize, serde::Serialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct $name {
            $(#[serde(deserialize_with = "linked_color")] pub $field: String,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: $default.to_string(),)* }
            }
        }

        impl $name {
            pub const TOKENS: &'static [&'static str] = &[$(stringify!($field)),*];

            pub fn values(&self) -> Vec<&str> {
                vec![$(self.$field.as_str()),*]
            }
        }
    };
}

tokens! {
    CodeTheme {
        keyword: "#E27373",
        storage: "#E27373",
        constant: "accent",
        operator: "#BDBDBD",
        punctuation: "muted",
        number: "accent",
        string: "#94B979",
        escape: "#97BEDC",
        comment: "muted",
        function: "#97BEDC",
        builtin: "#97BEDC",
        method: "#97BEDC",
        field: "none",
        parameter: "#BDBDBD",
        variable: "none",
        added: "#94B979",
        removed: "#E27373",
    }
}

tokens! {
    MarkdownTheme {
        heading: "#97BEDC",
        link: "accent",
        code: "#94B979",
        quote: "muted",
        list: "muted",
    }
}

macro_rules! base_theme {
    ($($field:ident: $default:literal,)*) => {
        pub const COLORS: [&str; 11] = [$(stringify!($field)),*];

        #[derive(Debug, Clone, Deserialize, serde::Serialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct Theme {
            $(#[serde(deserialize_with = "linked_color")] pub $field: String,)*
            pub code: CodeTheme,
            pub markdown: MarkdownTheme,
        }

        impl Default for Theme {
            fn default() -> Self {
                Self { $($field: $default.to_string(),)* code: CodeTheme::default(), markdown: MarkdownTheme::default() }
            }
        }

        impl Theme {
            pub fn named(&self, name: &str) -> Option<&str> {
                match name.trim() {
                    $(stringify!($field) => Some(&self.$field),)*
                    _ => None,
                }
            }
        }
    };
}

base_theme! {
    bg: "#121212", accent: "#BDDEAB", fg: "#DEDEDE", user: "fg", user_bg: "#1E1E1E",
    muted: "#929292", input: "user", input_bg: "#333333", border: "#333333",
    icons: "accent", error: "#E27373",
}

impl Theme {
    pub fn resolve<'a>(&'a self, value: &'a str) -> &'a str {
        let mut value = value.trim();
        for _ in 0..=COLORS.len() {
            match self.named(value) {
                Some(next) => value = next.trim(),
                None => return value,
            }
        }
        "none"
    }

    pub fn check(&self) -> Result<(), String> {
        for name in COLORS {
            let mut value = name;
            let mut hops = 0;
            while let Some(next) = self.named(value) {
                hops += 1;
                if hops > COLORS.len() {
                    return Err(format!("ui.theme.{name}: link cycle"));
                }
                value = next.trim();
            }
        }
        Ok(())
    }

    pub fn entries(&self) -> Vec<(String, &str)> {
        let mut out: Vec<(String, &str)> = COLORS
            .iter()
            .filter_map(|n| self.named(n).map(|v| (n.to_string(), v)))
            .collect();
        let groups = [
            ("code", CodeTheme::TOKENS, self.code.values()),
            ("markdown", MarkdownTheme::TOKENS, self.markdown.values()),
        ];
        for (group, names, values) in groups {
            for (name, value) in names.iter().zip(values) {
                out.push((format!("{group}.{name}"), value));
            }
        }
        out
    }
}

pub fn linked_color<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = String::deserialize(d)?;
    if !COLORS.contains(&s.trim()) && !valid_color(&s) {
        return Err(D::Error::custom(format!("invalid color `{s}`")));
    }
    Ok(s)
}

const NAMED_COLORS: &str = "none black red green yellow blue magenta cyan white grey gray dark_grey dark_gray bright_red bright_green bright_yellow bright_blue bright_magenta bright_cyan bright_white";

pub fn valid_color(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    if NAMED_COLORS.split_whitespace().any(|c| c == s) || s.parse::<u8>().is_ok() {
        return true;
    }
    s.strip_prefix('#')
        .is_some_and(|hex| matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit()))
}
