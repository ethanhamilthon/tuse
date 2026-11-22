use tuse_tui::{Field, Tone, ask, say};

/// `tuse connect`: asks for a provider, fetches its models and saves it to the database.
pub fn run(theme: tuse_config::Theme) -> Result<(), String> {
    let taken: Vec<String> = tuse_core::providers().into_iter().map(|c| c.name).collect();

    let url = |v: &str| {
        if v.starts_with("http://") || v.starts_with("https://") {
            Ok(())
        } else {
            Err("the base url must start with http:// or https://".to_string())
        }
    };
    let key = |v: &str| {
        if v.is_empty() {
            Err("the api key can't be empty".to_string())
        } else {
            Ok(())
        }
    };
    let name = |v: &str| {
        if v.is_empty() || v.contains(['/', ' ']) {
            Err("the name can't be empty or contain `/` or spaces".to_string())
        } else if taken.iter().any(|t| t == v) {
            Err(format!(
                "`{v}` already exists; `tuse disconnect {v}` removes it"
            ))
        } else {
            Ok(())
        }
    };
    let fields = [
        Field {
            label: "Base URL",
            secret: false,
            check: &url,
        },
        Field {
            label: "API key ($ENV works too)",
            secret: true,
            check: &key,
        },
        Field {
            label: "Name",
            secret: false,
            check: &name,
        },
    ];
    let io = |e: std::io::Error| e.to_string();
    let Some(a) = ask(&theme, &fields).map_err(io)? else {
        return Ok(());
    };
    let (base_url, api_key, name) = (&a[0], &a[1], &a[2]);

    say(&theme, Tone::Notice, "Fetching models...").map_err(io)?;
    let client = tuse_api::Client::new(base_url, &tuse_config::env_value(api_key));
    let models = client
        .models()
        .map_err(|e| format!("can't fetch models from {base_url}: {e}"))?;
    tuse_core::add_provider(&tuse_config::Connect {
        name: name.clone(),
        base_url: base_url.clone(),
        api_key: api_key.clone(),
        modes: vec![],
    })?;
    let msg = format!("Received {} models. Saved `{name}`.", models.len());
    say(&theme, Tone::Notice, &msg).map_err(io)
}

/// `tuse disconnect NAME`: removes a provider.
pub fn remove(name: &str) -> Result<(), String> {
    if tuse_core::remove_provider(name)? {
        println!("Removed `{name}`");
        Ok(())
    } else {
        Err(format!("no provider `{name}`"))
    }
}

/// `tuse providers`: the saved providers, keys hidden.
pub fn list() {
    for p in tuse_core::providers() {
        println!("{}\t{}", p.name, p.base_url);
    }
}
