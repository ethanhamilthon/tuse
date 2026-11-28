use crate::{Connect, env_value};
use tuse_api::ApiType;

impl Connect {
    pub fn resolve(&self) -> Result<Self, String> {
        if self.name.is_empty()
            || self.name.contains('/')
            || self.name.chars().any(char::is_whitespace)
        {
            return Err("the name can't be empty or contain `/` or whitespace".into());
        }
        if !self.base_url.starts_with("http://") && !self.base_url.starts_with("https://") {
            return Err("the base URL must start with http:// or https://".into());
        }
        if ApiType::parse(&self.api).is_err() {
            return Err(format!(
                "invalid API type (supported: {})",
                ApiType::ALL.join(", ")
            ));
        }
        let mut resolved = self.clone();
        resolved.api_key = env_value(&self.api_key);
        if resolved.api_key.trim().is_empty() || !valid_value(&resolved.api_key) {
            return Err("API key is unset, empty, or contains invalid header characters".into());
        }
        resolved.headers.clear();
        for (name, raw) in &self.headers {
            let name = name.to_ascii_lowercase();
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            {
                return Err("invalid header name".into());
            }
            if matches!(
                name.as_str(),
                "authorization"
                    | "x-api-key"
                    | "x-goog-api-key"
                    | "anthropic-version"
                    | "content-type"
                    | "host"
                    | "content-length"
                    | "transfer-encoding"
                    | "connection"
                    | "expect"
                    | "trailer"
                    | "te"
                    | "upgrade"
                    | "proxy-authorization"
                    | "proxy-connection"
                    | "keep-alive"
            ) {
                return Err(format!("header `{name}` is reserved"));
            }
            let value = env_value(raw);
            if value.trim().is_empty() || !valid_value(&value) {
                return Err(format!(
                    "header `{name}` is unset, empty, or contains invalid characters"
                ));
            }
            if resolved.headers.insert(name.clone(), value).is_some() {
                return Err(format!("duplicate header `{name}`"));
            }
        }
        Ok(resolved)
    }
}

fn valid_value(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b == b'\t' || (b >= b' ' && b != 0x7f))
}
