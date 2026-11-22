use serde::Deserialize;

/// Where Tuse runs: `tui` or `json` (`tuse json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Tui,
    Json,
}

pub fn mode_matches(modes: &[Mode], mode: Mode) -> bool {
    modes.is_empty() || modes.contains(&mode)
}

impl crate::Connect {
    pub fn matches(&self, mode: Mode) -> bool {
        mode_matches(&self.modes, mode)
    }
}

impl crate::Function {
    pub fn matches(&self, mode: Mode) -> bool {
        mode_matches(&self.modes, mode)
    }
}

impl crate::Hook {
    pub fn matches(&self, mode: Mode) -> bool {
        mode_matches(&self.modes, mode)
    }
}

/// Plain string or `{text, modes}` (used by `systems` and `prompts`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum ModeEntry {
    Text(String),
    Full {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        modes: Vec<Mode>,
    },
}

impl ModeEntry {
    pub fn text(&self) -> &str {
        match self {
            Self::Text(s) | Self::Full { text: s, .. } => s,
        }
    }

    pub fn modes(&self) -> &[Mode] {
        match self {
            Self::Text(_) => &[],
            Self::Full { modes, .. } => modes,
        }
    }

    pub fn matches(&self, mode: Mode) -> bool {
        mode_matches(self.modes(), mode)
    }
}

impl From<&str> for ModeEntry {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<String> for ModeEntry {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl<'de> Deserialize<'de> for ModeEntry {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = ModeEntry;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string or {text, modes}")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ModeEntry, E> {
                Ok(ModeEntry::Text(v.to_string()))
            }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<ModeEntry, E> {
                Ok(ModeEntry::Text(v))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<ModeEntry, A::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Full {
                    text: String,
                    #[serde(default)]
                    modes: Vec<Mode>,
                }
                let f = Full::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(ModeEntry::Full {
                    text: f.text,
                    modes: f.modes,
                })
            }
        }
        d.deserialize_any(V)
    }
}
