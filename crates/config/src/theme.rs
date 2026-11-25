use serde::Deserialize;
use serde::de::{Deserializer, Error};

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    #[serde(deserialize_with = "color")]
    pub bg: String,
    #[serde(deserialize_with = "color")]
    pub surface: String,
    #[serde(deserialize_with = "color")]
    pub fg: String,
    #[serde(deserialize_with = "color")]
    pub dim: String,
    #[serde(deserialize_with = "color")]
    pub muted: String,
    #[serde(deserialize_with = "color")]
    pub border: String,
    #[serde(deserialize_with = "color")]
    pub accent: String,
    #[serde(deserialize_with = "color")]
    pub ok: String,
    #[serde(deserialize_with = "color")]
    pub err: String,
    #[serde(deserialize_with = "color")]
    pub info: String,
}

impl Default for Theme {
    fn default() -> Self {
        let s = |v: &str| v.to_string();
        Self {
            bg: s("#121212"),
            surface: s("#1E1E1E"),
            fg: s("#DEDEDE"),
            dim: s("#BDBDBD"),
            muted: s("#929292"),
            border: s("#333333"),
            accent: s("#BDDEAB"),
            ok: s("#94B979"),
            err: s("#E27373"),
            info: s("#97BEDC"),
        }
    }
}

pub fn valid_color(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    if matches!(
        s.as_str(),
        "none"
            | "accent"
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

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiSettings {
    pub fold_strings: bool,
    #[serde(deserialize_with = "lines")]
    pub tool_line: i32,
    #[serde(deserialize_with = "lines")]
    pub diff_line: i32,
    #[serde(deserialize_with = "lines")]
    pub output_line: i32,
    #[serde(deserialize_with = "lines")]
    pub reasoning_line: i32,
    #[serde(deserialize_with = "crate::de::int::<_, _, 1, { u16::MAX as i64 }>")]
    pub scroll_step: u16,
    #[serde(deserialize_with = "crate::de::int::<_, _, 0, { u16::MAX as i64 }>")]
    pub pad_x: u16,
    #[serde(deserialize_with = "crate::de::int::<_, _, 0, { u16::MAX as i64 }>")]
    pub message_gap: u16,
    #[serde(deserialize_with = "crate::de::int::<_, _, 1, { i32::MAX as i64 }>")]
    pub list_max_height: usize,
    #[serde(deserialize_with = "crate::de::rows")]
    pub input_max_height: i32,
    #[serde(deserialize_with = "crate::names::history_roles")]
    pub hide_history: Vec<String>,
    #[serde(deserialize_with = "crate::names::model_scope")]
    pub scope_models: Vec<String>,
}

fn lines<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    crate::de::int::<D, i32, -1, { i32::MAX as i64 }>(d)
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            fold_strings: true,
            tool_line: -1,
            diff_line: 12,
            output_line: 1,
            reasoning_line: -1,
            scroll_step: 3,
            pad_x: 0,
            message_gap: 1,
            list_max_height: 16,
            input_max_height: -1,
            hide_history: vec![],
            scope_models: vec![],
        }
    }
}

pub fn limit(setting: i32, len: usize) -> usize {
    usize::try_from(setting).map_or(len, |n| n.min(len))
}

pub fn color<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = String::deserialize(d)?;
    if !valid_color(&s) {
        return Err(D::Error::custom(format!("invalid color `{s}`")));
    }
    Ok(s)
}
