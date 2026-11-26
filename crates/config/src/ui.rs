use crate::theme::Theme;
use serde::Deserialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputStyle {
    #[default]
    Boxed,
    Lines,
    Fill,
    Bar,
}

impl InputStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Boxed => "boxed",
            Self::Lines => "lines",
            Self::Fill => "fill",
            Self::Bar => "bar",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ui {
    pub lines: Lines,
    pub input: InputStyle,
    pub space: Space,
    pub theme: Theme,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Lines {
    #[serde(deserialize_with = "count")]
    pub tool: i32,
    #[serde(deserialize_with = "count")]
    pub output: i32,
    #[serde(deserialize_with = "count")]
    pub diff: i32,
    #[serde(deserialize_with = "count")]
    pub reasoning: i32,
    #[serde(deserialize_with = "count")]
    pub strings: i32,
    #[serde(deserialize_with = "crate::de::rows")]
    pub input: i32,
    #[serde(deserialize_with = "crate::de::int::<_, _, 1, { i32::MAX as i64 }>")]
    pub list: usize,
}

impl Default for Lines {
    fn default() -> Self {
        Self {
            tool: -1,
            output: 1,
            diff: 12,
            reasoning: -1,
            strings: 1,
            input: 10,
            list: 16,
        }
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Space {
    #[serde(deserialize_with = "crate::de::int::<_, _, 0, { u16::MAX as i64 }>")]
    pub padding: u16,
    #[serde(deserialize_with = "crate::de::int::<_, _, 0, { u16::MAX as i64 }>")]
    pub steps: u16,
    #[serde(deserialize_with = "crate::de::int::<_, _, 0, { u16::MAX as i64 }>")]
    pub messages: u16,
}

impl Default for Space {
    fn default() -> Self {
        Self {
            padding: 0,
            steps: 1,
            messages: 1,
        }
    }
}

fn count<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    crate::de::int::<D, i32, -1, { i32::MAX as i64 }>(d)
}

pub fn limit(setting: i32, len: usize) -> usize {
    usize::try_from(setting).map_or(len, |n| n.min(len))
}
