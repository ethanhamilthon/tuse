use crate::proto::{Fail, Reply, only_keys, opt_bool, opt_str, opt_u64, str_list, str_param};
use serde_json::{Value, json};
use std::sync::Arc;
use tuse_core::{ConnectInput, InputId, Plugins, ProviderPatch, Providers, Session, UserInput};

pub const PROTOCOL: u64 = 2;

pub const METHODS: &[&str] = &[
    "initialize",
    "turn.submit",
    "turn.withdraw",
    "turn.cancel",
    "session.state",
    "session.history",
    "session.list",
    "session.new",
    "session.fork",
    "model.set",
    "model.list",
    "providers.list",
    "providers.connect",
    "providers.update",
    "providers.disconnect",
    "plugins.list",
    "plugins.get",
    "plugins.add",
    "plugins.delete",
    "plugins.update",
    "config.get",
    "config.reload",
    "tasks.list",
    "tasks.get",
    "tasks.stop",
    "commands.list",
    "commands.run",
    "ui.state",
    "ui.reply",
    "ui.cancel",
    "shutdown",
];

fn state_json(s: &Session) -> Value {
    serde_json::to_value(s.snapshot(false)).unwrap_or(Value::Null)
}

pub fn switched(s: &Session) -> Value {
    json!({"session": state_json(s)})
}

pub fn initialize(s: &Session, p: &Value) -> Reply {
    match &p["protocolVersion"] {
        Value::Null => {}
        v if v.as_u64() == Some(PROTOCOL) => {}
        v => {
            return Err(Fail::params(format!(
                "unsupported protocolVersion {v}; this tuse speaks {PROTOCOL}"
            )));
        }
    }
    Ok(json!({
        "protocolVersion": PROTOCOL,
        "tuse": env!("CARGO_PKG_VERSION"),
        "session": state_json(s),
        "capabilities": {"methods": METHODS},
    }))
}

fn submit(s: &Arc<Session>, p: &Value) -> Reply {
    let text = str_param(p, "text")?;
    let prompts = s
        .resolve_prompts(&str_list(p, "prompts")?, true)
        .map_err(Fail::session)?;
    let (urls, paths): (Vec<String>, Vec<String>) = str_list(p, "images")?
        .into_iter()
        .partition(|f| f.starts_with("data:"));
    if urls.iter().any(|u| !tuse_core::is_valid_image_data_url(u)) {
        return Err(Fail::session("images must be image data URLs"));
    }
    let refs = tuse_core::Refs {
        prompts,
        images: (1..).zip(paths.into_iter().map(Into::into)).collect(),
        files: str_list(p, "files")?.into_iter().map(Into::into).collect(),
        cwd: s.cwd().to_path_buf(),
        strict: true,
    };
    let mut input: UserInput = tuse_core::build_input(&text, refs).map_err(Fail::session)?;
    input.images.extend(urls);
    if input.text.trim().is_empty() && input.context.is_empty() && input.images.is_empty() {
        return Err(Fail::session("no input"));
    }
    let (id, queued) = s.submit(input);
    Ok(json!({"input_id": id.number(), "queued": queued}))
}

macro_rules! reply {
    ($v:expr) => {
        serde_json::to_value($v).map_err(|e| Fail::session(e.to_string()))
    };
}

macro_rules! reply_res {
    ($r:expr) => {
        reply!($r.map_err(Fail::session)?)
    };
}

pub fn call(s: &Arc<Session>, method: &str, p: &Value) -> Reply {
    match method {
        "turn.submit" => submit(s, p),
        "turn.withdraw" => {
            let n = p["input_id"]
                .as_u64()
                .ok_or_else(|| Fail::params("`input_id` must be a number"))?;
            Ok(json!(s.withdraw_input(InputId::from_number(n))))
        }
        "turn.cancel" => {
            s.cancel();
            Ok(Value::Null)
        }
        "session.state" => reply!(s.snapshot(false)),
        "session.history" => reply!(s.history()),
        "session.list" => {
            let cwd = opt_str(p, "cwd")?.map_or_else(|| s.cwd().to_path_buf(), Into::into);
            reply!(tuse_core::Meta::list(&cwd))
        }
        "model.set" => {
            let model = str_param(p, "model")?;
            let effort = match &p["effort"] {
                Value::Null if model.is_empty() => "medium".to_string(),
                Value::Null => s.config().fallback_effort(&model),
                _ => str_param(p, "effort")?,
            };
            let persist = opt_bool(p, "persist")?.unwrap_or(true);
            s.select_model(&model, &effort, persist)
                .map(|_| Value::Null)
                .map_err(Fail::session)
        }
        "model.list" => {
            if !opt_bool(p, "cached")?.unwrap_or(false) {
                s.fetch_models();
            }
            Ok(json!(s.available_models()))
        }
        "providers.list" => {
            only_keys(p, &[])?;
            reply_res!(Providers::views())
        }
        "providers.connect" => {
            let input = serde_json::from_value::<ConnectInput>(p.clone())
                .map_err(|_| Fail::params("invalid provider options"))?;
            reply_res!(s.connect_provider(input))
        }
        "providers.update" => {
            let name = str_param(p, "name")?;
            let mut fields = p
                .as_object()
                .cloned()
                .ok_or_else(|| Fail::params("expected provider options"))?;
            fields.remove("name");
            let patch = serde_json::from_value::<ProviderPatch>(fields.into())
                .map_err(|_| Fail::params("invalid provider patch"))?;
            reply_res!(s.update_provider(&name, patch))
        }
        "providers.disconnect" => {
            only_keys(p, &["name"])?;
            let name = str_param(p, "name")?;
            Ok(json!({ "name": s.disconnect_provider(&name).map_err(Fail::session)? }))
        }
        "plugins.list" => {
            only_keys(p, &[])?;
            reply_res!(Plugins::list())
        }
        "plugins.get" => {
            only_keys(p, &["target"])?;
            reply_res!(Plugins::get(&str_param(p, "target")?))
        }
        "plugins.add" => {
            only_keys(p, &["source", "replace"])?;
            let (source, replace) = (
                str_param(p, "source")?,
                opt_bool(p, "replace")?.unwrap_or(false),
            );
            reply_res!(Plugins::add(&source, replace))
        }
        "plugins.delete" => {
            only_keys(p, &["target"])?;
            let name = Plugins::delete(&str_param(p, "target")?).map_err(Fail::session)?;
            Ok(json!({ "name": name }))
        }
        "plugins.update" => {
            only_keys(p, &["target"])?;
            let target = opt_str(p, "target")?;
            let updated = Plugins::update(target.as_deref()).map_err(Fail::session)?;
            Ok(json!({ "updated": updated }))
        }
        "config.get" => reply!(&*s.config()),
        "config.reload" => {
            only_keys(p, &[])?;
            s.reload().map(|_| Value::Null).map_err(Fail::session)
        }
        "tasks.list" => reply!(s.tasks()),
        "tasks.get" => {
            let id = str_param(p, "id")?;
            let lines = opt_u64(p, "lines")?.map_or(50, |n| n.max(1) as usize);
            let t = s
                .task(&id)
                .ok_or_else(|| Fail::session(format!("no task {id}")))?;
            let mut v = reply!(&t)?;
            v["output"] = json!(tuse_core::Tasks::tail(&t.log, lines));
            Ok(v)
        }
        "tasks.stop" => reply_res!(s.stop_task(&str_param(p, "id")?)),
        "commands.list" => reply!(
            s.config()
                .commands
                .iter()
                .map(|name| json!({ "name": name }))
                .collect::<Vec<_>>()
        ),
        "commands.run" => {
            only_keys(p, &["name"])?;
            let name = str_param(p, "name")?;
            s.run_command(&name, &[])
                .map(|_| Value::Null)
                .map_err(Fail::session)
        }
        "ui.state" => reply!(s.ui_snapshot()),
        "ui.reply" => {
            let id = p["id"]
                .as_u64()
                .ok_or_else(|| Fail::params("`id` must be a number"))?;
            let value = str_param(p, "value")?;
            s.ui_reply(id, Some(value))
                .map(|_| Value::Null)
                .map_err(Fail::session)
        }
        "ui.cancel" => {
            let id = p["id"]
                .as_u64()
                .ok_or_else(|| Fail::params("`id` must be a number"))?;
            s.ui_reply(id, None)
                .map(|_| Value::Null)
                .map_err(Fail::session)
        }
        _ => Err(Fail(
            crate::proto::NO_METHOD,
            format!("unknown method `{method}`"),
        )),
    }
}
