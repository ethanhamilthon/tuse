use tuse_tui::{Field, Tone, ask, say};

/// `tuse connect`: asks for a provider, fetches its models and saves it to the config.
pub fn run(config_arg: Option<std::path::PathBuf>) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let path =
        tuse_config::find(config_arg.as_deref(), &cwd).unwrap_or_else(tuse_config::global_path);
    // A config that does not exist yet is created by the save below.
    let config = if path.is_file() {
        tuse_config::load(Some(&path), tuse_config::Mode::Tui)
            .map_err(|e| format!("config: {e}"))?
            .config
    } else {
        tuse_config::Config::default()
    };
    let theme = config.active_theme();
    let taken: Vec<String> = config.connect.iter().map(|c| c.name.clone()).collect();

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
            Err(format!("`{v}` already exists in the config"))
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
    let client = tuse_api::Client::new(base_url, &tuse_config::expand(api_key));
    let models = client
        .models()
        .map_err(|e| format!("can't fetch models from {base_url}: {e}"))?;
    tuse_config::add_connect(&path, name, base_url, api_key)?;
    let msg = format!(
        "Received {} models. Saved `{name}` to {}.",
        models.len(),
        path.display()
    );
    say(&theme, Tone::Notice, &msg).map_err(io)
}
