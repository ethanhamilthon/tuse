use crate::storage::db;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tuse_api::{ApiType, Client};
use tuse_config::Connect;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderView {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub base: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderResult {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub base: String,
    pub models: usize,
    #[serde(skip)]
    pub(crate) model_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectInput {
    pub name: String,
    pub api: Option<String>,
    pub base: String,
    pub key: String,
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(default, deserialize_with = "optional_false")]
    pub replace: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderPatch {
    pub api: Option<String>,
    pub base: Option<String>,
    pub key: Option<String>,
    pub headers: Option<BTreeMap<String, String>>,
}

fn optional_false<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(Option::<bool>::deserialize(d)?.unwrap_or(false))
}

impl db::Providers {
    pub fn views() -> Result<Vec<ProviderView>, String> {
        Ok(Self::list()?
            .into_iter()
            .map(|p| ProviderView {
                kind: p.api_type().to_string(),
                name: p.name,
                base: p.base_url,
            })
            .collect())
    }

    pub fn has(name: &str) -> bool {
        Self::get(name).ok().flatten().is_some()
    }

    pub fn resolve_by_name(name: &str) -> Result<Option<Connect>, String> {
        match Self::get(name)? {
            Some(p) => Self::resolve(&p).map(Some),
            None => Ok(None),
        }
    }

    pub fn connect(input: ConnectInput) -> Result<ProviderResult, String> {
        let name = input.name.trim().to_string();
        if name.is_empty() || name.contains('/') || name.chars().any(char::is_whitespace) {
            return Err("the name can't be empty or contain `/` or whitespace".into());
        }
        if !input.replace && Self::get(&name)?.is_some() {
            return Err(format!(
                "provider `{name}` already exists; pass `replace = true` to overwrite it or use `providers.update`"
            ));
        }
        let api = match input.api {
            Some(raw) => ApiType::parse(&raw)?,
            None => ApiType::OpenAiChat,
        };
        let profile = Connect {
            name,
            base_url: input.base.trim().to_string(),
            api_key: input.key,
            api: api.as_str().to_string(),
            headers: input.headers.unwrap_or_default(),
        };
        probe_and_save(profile, api)
    }

    pub fn update(name: &str, patch: ProviderPatch) -> Result<ProviderResult, String> {
        let mut profile = Self::get(name)?.ok_or_else(|| format!("no provider `{name}`"))?;
        if let Some(api) = patch.api {
            profile.api = ApiType::parse(&api)?.as_str().to_string();
        }
        if let Some(base) = patch.base {
            profile.base_url = base.trim().to_string();
        }
        if let Some(key) = patch.key {
            profile.api_key = key;
        }
        if let Some(headers) = patch.headers {
            profile.headers = headers;
        }
        let api = ApiType::parse(profile.api_type())?;
        probe_and_save(profile, api)
    }

    pub fn disconnect(name: &str) -> Result<String, String> {
        if !Self::remove(name)? {
            return Err(format!("no provider `{name}`"));
        }
        Ok(name.to_string())
    }
}

fn probe_and_save(mut profile: Connect, api: ApiType) -> Result<ProviderResult, String> {
    let resolved = profile.resolve()?;
    let models = Client::new(&resolved.base_url, &resolved.api_key, api)
        .with_headers(resolved.headers.clone())
        .models()
        .map_err(|e| {
            let clean = redact_secrets(&e.to_string(), &profile, &resolved);
            format!("can't fetch models from {}: {clean}", resolved.base_url)
        })?;
    profile.headers = profile
        .headers
        .into_iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v))
        .collect();
    db::Providers::add(&profile)?;
    Ok(ProviderResult {
        name: profile.name,
        kind: api.as_str().to_string(),
        base: profile.base_url,
        models: models.len(),
        model_ids: models,
    })
}

pub(crate) fn redact_secrets(msg: &str, raw: &Connect, resolved: &Connect) -> String {
    let mut secrets: Vec<&str> = std::iter::once(raw.api_key.as_str())
        .chain(std::iter::once(resolved.api_key.as_str()))
        .chain(raw.headers.values().map(String::as_str))
        .chain(resolved.headers.values().map(String::as_str))
        .filter(|s| !s.is_empty())
        .collect();
    secrets.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    secrets.dedup();
    let mut out = msg.to_string();
    for s in secrets {
        out = if s.len() >= 4 {
            out.replace(s, "[redacted]")
        } else {
            replace_token(&out, s, "[redacted]")
        };
    }
    out
}

fn replace_token(haystack: &str, needle: &str, with: &str) -> String {
    let mut out = String::with_capacity(haystack.len());
    let mut rest = haystack;
    let mut prev: Option<char> = None;
    let bound = |c: Option<char>| c.is_none_or(|ch| !ch.is_ascii_alphanumeric());
    while let Some(pos) = rest.find(needle) {
        let before = &rest[..pos];
        let left = before.chars().next_back().or(prev);
        let after = &rest[pos + needle.len()..];
        if bound(left) && bound(after.chars().next()) {
            out.push_str(before);
            out.push_str(with);
            prev = with.chars().next_back();
            rest = after;
        } else {
            let step = pos + needle.chars().next().map_or(1, char::len_utf8);
            out.push_str(&rest[..step]);
            prev = rest[..step].chars().next_back();
            rest = &rest[step..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Providers;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    fn mock(status: &'static str, body: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(vec![]));
        let log = seen.clone();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let (mut r, mut hdrs) = (BufReader::new(s.try_clone().unwrap()), String::new());
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                    hdrs.push_str(&line.to_ascii_lowercase());
                }
                log.lock().unwrap().push(hdrs);
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (url, seen)
    }

    #[test]
    fn provider_service_probe_env_headers_patch_disconnect_and_redaction() {
        let _g = crate::storage::ENV_LOCK.lock().unwrap();
        let home = std::env::temp_dir().join(format!("tuse-prov-svc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        unsafe {
            std::env::set_var("TUSE_HOME", &home);
            std::env::set_var("TUSE_TEST_PROV_KEY", "sk-live-secret-999");
        }
        let (ok_url, seen) = mock("200 OK", r#"{"data":[{"id":"m1"},{"id":"m2"}]}"#);
        let (anth_url, _) = mock("200 OK", r#"{"data":[{"id":"claude-1"}]}"#);
        let (gem_url, _) = mock("200 OK", r#"{"models":[{"name":"models/gemini-2.5-pro"}]}"#);
        let (bad_url, _) = mock("401 Unauthorized", "invalid key sk-live-secret-999");
        let (short_bad_url, _) = mock(
            "401 Unauthorized",
            "Bearer k1 rejected on 127.0.0.1 token=abc",
        );

        let mut headers = BTreeMap::new();
        headers.insert("X-Tenant".into(), "demo".into());
        headers.insert("X-Token".into(), "$TUSE_TEST_PROV_KEY".into());
        let res = Providers::connect(ConnectInput {
            name: "gw".into(),
            api: Some("openai-responses".into()),
            base: ok_url.clone(),
            key: "$TUSE_TEST_PROV_KEY".into(),
            headers: Some(headers),
            replace: false,
        })
        .unwrap();
        assert_eq!(res.models, 2);
        assert_eq!(res.kind, "openai-responses");

        for (name, api, base) in [
            ("p_chat", "openai-chat", ok_url.clone()),
            ("p_anth", "anthropic-messages", anth_url),
            ("p_gem", "google-gemini", gem_url),
        ] {
            let r = Providers::connect(ConnectInput {
                name: name.into(),
                api: Some(api.into()),
                base,
                key: "k-proto".into(),
                headers: None,
                replace: false,
            })
            .unwrap();
            assert_eq!(r.kind, api);
            assert!(r.models >= 1);
            assert_eq!(Providers::disconnect(name).unwrap(), name);
        }

        for reserved in [
            "Authorization",
            "Content-Type",
            "Host",
            "X-Api-Key",
            "Anthropic-Version",
            "X-Goog-Api-Key",
        ] {
            let mut h = BTreeMap::new();
            h.insert(reserved.into(), "val".into());
            let err = Providers::connect(ConnectInput {
                name: "bad_hdr".into(),
                api: None,
                base: ok_url.clone(),
                key: "k".into(),
                headers: Some(h),
                replace: false,
            })
            .unwrap_err();
            assert!(err.contains("reserved"), "{reserved}: {err}");
        }
        let mut crlf_hdr = BTreeMap::new();
        crlf_hdr.insert("X-Custom".into(), "ok\r\nX-Injected: bad".into());
        assert!(
            Providers::connect(ConnectInput {
                name: "bad_crlf".into(),
                api: None,
                base: ok_url.clone(),
                key: "k".into(),
                headers: Some(crlf_hdr),
                replace: false,
            })
            .is_err()
        );
        assert!(
            Providers::connect(ConnectInput {
                name: "bad_key_crlf".into(),
                api: None,
                base: ok_url.clone(),
                key: "k\r\nX: 1".into(),
                headers: None,
                replace: false,
            })
            .is_err()
        );

        let mut short_hdr = BTreeMap::new();
        short_hdr.insert("X-Short".into(), "abc".into());
        let short_err = Providers::connect(ConnectInput {
            name: "short_sec".into(),
            api: None,
            base: short_bad_url,
            key: "k1".into(),
            headers: Some(short_hdr),
            replace: false,
        })
        .unwrap_err();
        assert!(
            !short_err.contains("Bearer k1")
                && !short_err.contains("token=abc")
                && short_err.contains("127.0.0.1")
                && short_err.contains("[redacted]"),
            "{short_err}"
        );

        let stored = Providers::get("gw").unwrap().unwrap();
        assert_eq!(stored.api_key, "$TUSE_TEST_PROV_KEY");
        assert_eq!(stored.headers["x-token"], "$TUSE_TEST_PROV_KEY");
        assert!(seen.lock().unwrap()[0].contains("x-token: sk-live-secret-999"));

        let dup = Providers::connect(ConnectInput {
            name: "gw".into(),
            api: None,
            base: ok_url.clone(),
            key: "other".into(),
            headers: None,
            replace: false,
        })
        .unwrap_err();
        assert!(
            dup.contains("replace") && dup.contains("providers.update"),
            "{dup}"
        );

        assert!(
            Providers::connect(ConnectInput {
                name: "gw".into(),
                api: Some("openai-chat".into()),
                base: bad_url.clone(),
                key: "sk-live-secret-999".into(),
                headers: None,
                replace: true,
            })
            .is_err()
        );
        let kept = Providers::get("gw").unwrap().unwrap();
        assert_eq!(kept.base_url, ok_url);
        assert_eq!(kept.api, "openai-responses");

        let mut dup_hdrs = BTreeMap::new();
        dup_hdrs.insert("X-Dup".into(), "1".into());
        dup_hdrs.insert("x-dup".into(), "2".into());
        assert!(
            Providers::connect(ConnectInput {
                name: "gw2".into(),
                api: None,
                base: ok_url.clone(),
                key: "k".into(),
                headers: Some(dup_hdrs),
                replace: false,
            })
            .is_err()
        );

        let probe_err = Providers::update(
            "gw",
            ProviderPatch {
                base: Some(bad_url),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(!probe_err.contains("sk-live-secret-999"), "{probe_err}");
        assert_eq!(Providers::get("gw").unwrap().unwrap().base_url, ok_url);

        let updated = Providers::update(
            "gw",
            ProviderPatch {
                key: Some("rotated-secret-value".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.base, ok_url);
        assert_eq!(updated.kind, "openai-responses");
        assert!(
            seen.lock()
                .unwrap()
                .last()
                .unwrap()
                .contains("bearer rotated-secret-value")
        );
        assert!(
            seen.lock()
                .unwrap()
                .last()
                .unwrap()
                .contains("x-tenant: demo")
        );

        let views_json = Providers::json().unwrap();
        for secret in [
            "rotated-secret-value",
            "sk-live-secret-999",
            "x-tenant",
            "headers",
        ] {
            assert!(!views_json.contains(secret), "{views_json}");
        }
        assert_eq!(Providers::disconnect("gw").unwrap(), "gw");
        assert!(Providers::get("gw").unwrap().is_none());
        assert!(Providers::disconnect("gw").is_err());

        unsafe {
            std::env::remove_var("TUSE_HOME");
            std::env::remove_var("TUSE_TEST_PROV_KEY");
        }
        let _ = std::fs::remove_dir_all(&home);
    }
}
