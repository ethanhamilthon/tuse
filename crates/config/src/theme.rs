use serde::Deserialize;
use serde::de::{Deserializer, Error};

pub const COLORS: [&str; 11] = [
    "bg", "accent", "fg", "user", "user_bg", "muted", "input", "input_bg", "border", "icons",
    "error",
];

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

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    #[serde(deserialize_with = "linked_color")]
    pub bg: String,
    #[serde(deserialize_with = "linked_color")]
    pub accent: String,
    #[serde(deserialize_with = "linked_color")]
    pub fg: String,
    #[serde(deserialize_with = "linked_color")]
    pub user: String,
    #[serde(deserialize_with = "linked_color")]
    pub user_bg: String,
    #[serde(deserialize_with = "linked_color")]
    pub muted: String,
    #[serde(deserialize_with = "linked_color")]
    pub input: String,
    #[serde(deserialize_with = "linked_color")]
    pub input_bg: String,
    #[serde(deserialize_with = "linked_color")]
    pub border: String,
    #[serde(deserialize_with = "linked_color")]
    pub icons: String,
    #[serde(deserialize_with = "linked_color")]
    pub error: String,
    pub code: CodeTheme,
    pub markdown: MarkdownTheme,
}

impl Default for Theme {
    fn default() -> Self {
        let s = |v: &str| v.to_string();
        Self {
            bg: s("#121212"),
            accent: s("#BDDEAB"),
            fg: s("#DEDEDE"),
            user: s("fg"),
            user_bg: s("#1E1E1E"),
            muted: s("#929292"),
            input: s("user"),
            input_bg: s("#333333"),
            border: s("#333333"),
            icons: s("accent"),
            error: s("#E27373"),
            code: CodeTheme::default(),
            markdown: MarkdownTheme::default(),
        }
    }
}

impl Theme {
    pub fn named(&self, name: &str) -> Option<&str> {
        Some(match name.trim() {
            "bg" => &self.bg,
            "accent" => &self.accent,
            "fg" => &self.fg,
            "user" => &self.user,
            "user_bg" => &self.user_bg,
            "muted" => &self.muted,
            "input" => &self.input,
            "input_bg" => &self.input_bg,
            "border" => &self.border,
            "icons" => &self.icons,
            "error" => &self.error,
            _ => return None,
        })
    }

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

pub fn valid_color(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    if matches!(
        s.as_str(),
        "none"
            | "black"
            | "red"
            | "green"
            | "yellow"
            | "blue"
            | "magenta"
            | "cyan"
            | "white"
            | "grey"
            | "gray"
            | "dark_grey"
            | "dark_gray"
            | "bright_red"
            | "bright_green"
            | "bright_yellow"
            | "bright_blue"
            | "bright_magenta"
            | "bright_cyan"
            | "bright_white"
    ) {
        return true;
    }
    if s.parse::<u8>().is_ok() {
        return true;
    }
    if let Some(hex) = s.strip_prefix('#') {
        return matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    false
}
