use crate::args::Args;

const USAGE: &str = "tuse connect --base URL --key '$ENV' --name NAME [--api TYPE]";

pub fn run(args: &Args) -> Result<(), String> {
    let required = |flag| {
        args.value(&[flag])
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| format!("{flag} is required\nusage: {USAGE}"))
    };
    let base_url = required("--base")?;
    let api_key = required("--key")?;
    let name = required("--name")?;
    let api = match args.value(&["--api"]) {
        Some(raw) => tuse_api::ApiType::parse(&raw)?,
        None => tuse_api::ApiType::OpenAiChat,
    };
    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return Err("the base URL must start with http:// or https://".into());
    }
    if name.contains('/') || name.chars().any(char::is_whitespace) {
        return Err("the name can't contain `/` or whitespace".into());
    }
    if tuse_core::providers().iter().any(|p| p.name == name) {
        return Err(format!(
            "`{name}` already exists; `tuse disconnect {name}` removes it"
        ));
    }
    let key = tuse_config::env_value(&api_key);
    if key.trim().is_empty() {
        return Err("the API key environment variable is unset or empty".into());
    }
    let models = tuse_api::Client::new(&base_url, &key, api)
        .models()
        .map_err(|e| format!("can't fetch models from {base_url}: {e}"))?;
    tuse_core::add_provider(&tuse_config::Connect {
        name: name.clone(),
        base_url,
        api_key,
        api: api.as_str().to_string(),
    })?;
    println!("Received {} models. Saved `{name}`.", models.len());
    Ok(())
}

pub fn remove(name: &str) -> Result<(), String> {
    if !tuse_core::remove_provider(name)? {
        return Err(format!("no provider `{name}`"));
    }
    println!("Removed `{name}`");
    Ok(())
}

pub fn list(as_json: bool) -> Result<(), String> {
    if as_json {
        println!("{}", tuse_core::providers_json());
    } else {
        for p in tuse_core::providers() {
            println!("{}\t{}", p.name, p.base_url);
        }
    }
    Ok(())
}
