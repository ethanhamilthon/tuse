use crate::args::Args;
use std::io::Read;

const USAGE: &str = "tuse connect --base URL (--key '$ENV' | --key-stdin) --name NAME [--api TYPE] [--header HEADER]... [--replace]";

pub fn run(args: &Args) -> Result<(), String> {
    let required = |flag| {
        args.value(&[flag])
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| format!("{flag} is required\nusage: {USAGE}"))
    };
    let base_url = required("--base")?;
    let name = required("--name")?;
    let key_stdin = args.has(&["--key-stdin"]);
    let key_arg = args.value(&["--key"]);
    if key_stdin && key_arg.is_some() {
        return Err(format!(
            "pass either --key or --key-stdin, not both\nusage: {USAGE}"
        ));
    }
    let api_key = if key_stdin {
        let mut raw = String::new();
        std::io::stdin()
            .read_to_string(&mut raw)
            .map_err(|e| format!("failed to read --key-stdin: {e}"))?;
        let trimmed = raw.trim_end_matches(['\r', '\n']).to_string();
        if trimmed.trim().is_empty() {
            return Err("the API key from stdin is empty".into());
        }
        trimmed
    } else {
        key_arg
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| format!("--key or --key-stdin is required\nusage: {USAGE}"))?
    };
    let api = match args.value(&["--api"]) {
        Some(raw) => tuse_api::ApiType::parse(&raw)?,
        None => tuse_api::ApiType::OpenAiChat,
    };
    let replace = args.has(&["--replace"]);
    if !replace && tuse_core::Providers::list()?.iter().any(|p| p.name == name) {
        return Err(format!(
            "`{name}` already exists; `tuse disconnect {name}` removes it"
        ));
    }
    let mut headers = std::collections::BTreeMap::new();
    for header in args.values("--header") {
        let (name, value) = header
            .split_once(':')
            .ok_or("--header expects 'Name: value'")?;
        let name = name.trim_matches([' ', '\t']).to_ascii_lowercase();
        if headers
            .insert(name.clone(), value.trim_matches([' ', '\t']).to_owned())
            .is_some()
        {
            return Err(format!("duplicate header `{name}`"));
        }
    }
    let profile = tuse_config::Connect {
        name: name.clone(),
        base_url,
        api_key,
        api: api.as_str().to_string(),
        headers,
    };
    let resolved = profile.resolve()?;
    let models = tuse_api::Client::new(&resolved.base_url, &resolved.api_key, api)
        .with_headers(resolved.headers)
        .models()
        .map_err(|e| format!("can't fetch models from {}: {e}", resolved.base_url))?;
    tuse_core::Providers::add(&profile)?;
    println!("Received {} models. Saved `{name}`.", models.len());
    Ok(())
}

pub fn remove(name: &str) -> Result<(), String> {
    if !tuse_core::Providers::remove(name)? {
        return Err(format!("no provider `{name}`"));
    }
    println!("Removed `{name}`");
    Ok(())
}

pub fn list(as_json: bool) -> Result<(), String> {
    if as_json {
        println!("{}", tuse_core::Providers::json()?);
    } else {
        for p in tuse_core::Providers::list()? {
            println!("{}\t{}", p.name, p.base_url);
        }
    }
    Ok(())
}
