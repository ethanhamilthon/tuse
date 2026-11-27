use crate::convo::record::Record;
use crate::convo::wire;
use crate::mode::{Options, Start};
use crate::session::{Session, State};
use crate::storage::store::{Meta, SessionDir};
use crate::storage::tasks::Tasks;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tuse_config::Config;

pub(crate) struct Prepared {
    opts: Options,
    id: String,
    dir: SessionDir,
    vm: tuse_lua::Vm,
    weak: crate::session::lua::host::Slot,
    config: Config,
}

pub fn open(opts: Options) -> Result<Arc<Session>, String> {
    prepare(opts)?.start()
}

pub fn switch(session: &Arc<Session>, resume: Option<String>) -> Result<Arc<Session>, String> {
    if resume.as_deref() == Some(session.id.as_str()) {
        return Err(format!("session {} is already open", session.id));
    }
    let start = if resume.is_some() {
        Start::Resume
    } else {
        Start::New
    };
    let next = prepare(Options {
        cwd: session.cwd.clone(),
        config: session.config_arg.clone(),
        mode: session.mode,
        resume,
        start,
    })?;
    session.close_for(Some("session_close"));
    next.start()
}

pub fn fork(session: &Arc<Session>, index: usize) -> Result<(Arc<Session>, String), String> {
    let (records, draft) = session.with_records(|records| fork_records(records, index))?;
    let source = Meta::read(&session.id)?.ok_or_else(|| format!("no session {}", session.id))?;
    let id = crate::id::new_session_id();
    let meta = Meta {
        id: id.clone(),
        title: format!("[fork] {}", source.title).trim_end().to_string(),
        created: crate::id::now(),
        ..source
    };
    SessionDir::create_fork(&meta, &records)?;
    Ok((switch(session, Some(id))?, draft))
}

pub(crate) fn check_fork(records: &[Record], index: usize) -> Result<(), String> {
    match records.get(index) {
        Some(Record::User { .. } | Record::Agent { .. }) => Ok(()),
        Some(_) => Err(format!(
            "record {} is not a user or agent message",
            index + 1
        )),
        None => Err(format!("no record {}", index + 1)),
    }
}

pub(crate) fn check_resume(session: &Session, id: &str) -> Result<(), String> {
    if id == session.id {
        return Err(format!("session {id} is already open"));
    }
    let meta = Meta::read(id)?.ok_or_else(|| format!("no session {id}"))?;
    if !meta.started {
        return Err(format!("session {id} has no messages"));
    }
    if meta.cwd != session.cwd.display().to_string() {
        return Err(format!("session {id} belongs to another folder"));
    }
    Ok(())
}

fn fork_records(records: &[Record], index: usize) -> Result<(Vec<Record>, String), String> {
    let (end, draft) = match records.get(index) {
        Some(Record::User { text, .. }) => (index, text.clone()),
        Some(Record::Agent { tool_calls, .. }) => {
            let mut end = index + 1;
            for (at, record) in records.iter().enumerate().skip(index + 1) {
                match record {
                    Record::User { .. } | Record::Agent { .. } => break,
                    Record::Lua { id, .. } if tool_calls.iter().any(|c| c.id == *id) => {
                        end = at + 1;
                    }
                    _ => {}
                }
            }
            (end, String::new())
        }
        Some(_) => return Err(format!("record {index} is not a user or agent message")),
        None => return Err(format!("no record {index}")),
    };
    let mut copy: Vec<Record> = records[..end]
        .iter()
        .filter(|r| !matches!(r, Record::Usage { .. }))
        .cloned()
        .collect();
    copy.extend(wire::missing_results(&copy));
    Ok((copy, draft))
}

fn prepare(mut opts: Options) -> Result<Prepared, String> {
    let restored = opts.resume.is_some();
    let id = opts
        .resume
        .clone()
        .unwrap_or_else(crate::id::new_session_id);
    let (vm, weak, config) = if restored {
        let meta =
            crate::storage::store::Meta::read(&id)?.ok_or_else(|| format!("no session {id}"))?;
        let cwd = PathBuf::from(&meta.cwd);
        if cwd.is_dir() {
            opts.cwd = cwd;
        }
        crate::session::lua::config_lua::start_vm(&opts)?
    } else {
        crate::session::lua::config_lua::start_vm(&opts)?
    };
    let meta = (!restored).then(|| {
        let model = crate::storage::Prefs::model();
        let effort = crate::storage::Prefs::effort_for(&config, &model);
        Meta {
            id: id.clone(),
            cwd: opts.cwd.display().to_string(),
            created: crate::id::now(),
            model,
            effort,
            ..Default::default()
        }
    });
    let dir = SessionDir::open(&id, meta.as_ref())?;
    Ok(Prepared {
        opts,
        id,
        dir,
        vm,
        weak,
        config,
    })
}

impl Prepared {
    pub(crate) fn start(self) -> Result<Arc<Session>, String> {
        let Prepared {
            opts,
            id,
            dir,
            vm,
            weak,
            config,
        } = self;
        let restored = opts.resume.is_some();
        dir.system(&Record::System {
            text: crate::prompt::system_prompt(&config),
        })?;
        if restored {
            let cache = SessionDir::read(&id)?;
            let missing = wire::missing_results(&cache.records);
            if !missing.is_empty() {
                dir.append(&missing, &config)?;
            }
        }
        let st = State {
            activity: Default::default(),
            cache: None,
            pending_history: None,
            pending_switch: None,
            tool_step: false,
            busy: false,
            generation: 0,
            cancel: Arc::default(),
            tool_handoff: None,
            queue: vec![],
            next_input: 0,
            turn_start: Instant::now(),
            calls: 0,
            status: String::new(),
            partial: String::new(),
            reasoning: String::new(),
            streaming: false,
            remote: vec![],
        };
        let config = Arc::new(config);
        let path = dir.path();
        let s = Arc::new_cyclic(|me| {
            *weak.lock().unwrap() = me.clone();
            Session {
                id,
                cwd: opts.cwd,
                mode: opts.mode,
                tasks: Tasks::new(path.join("tasks")),
                dir: Mutex::new(Some(dir)),
                config: RwLock::new((0, config)),
                config_arg: opts.config,
                state: Mutex::new(st),
                subs: Mutex::default(),
                vm,
                hooks_on: AtomicBool::new(true),
                hook_runs: Default::default(),
                ui: Mutex::default(),
                cmd_runs: Mutex::default(),
                http: tuse_api::default_agent(),
                next_lua: AtomicU64::new(0),
                task_lua: Mutex::default(),
                jobs: Mutex::default(),
                reload_lock: Mutex::default(),
            }
        });
        s.lifecycle(opts.start == Start::Launch, restored);
        Ok(s)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tuse_api::{ToolCall, Usage};

    fn history() -> Vec<Record> {
        let user = |t: &str| Record::User {
            text: t.into(),
            context: String::new(),
            images: vec![],
        };
        vec![
            Record::System { text: "s".into() },
            user("one"),
            Record::Agent {
                text: String::new(),
                reasoning: String::new(),
                tool_calls: vec![ToolCall {
                    id: "c".into(),
                    name: "lua".into(),
                    arguments: "{}".into(),
                }],
                replay: None,
                cancelled: false,
            },
            Record::usage("m".into(), Usage::default(), false),
            Record::Lua {
                id: "c".into(),
                code: String::new(),
                output: "ok".into(),
                exit: Some(0),
                images: vec![],
                diffs: vec![],
            },
            user("two"),
        ]
    }

    #[test]
    fn fork_cuts_before_a_user_message_and_after_an_agent_message_with_its_results() {
        let records = history();
        let (copy, draft) = fork_records(&records, 5).unwrap();
        assert_eq!((copy.len(), draft.as_str()), (4, "two"));
        let (copy, draft) = fork_records(&records, 2).unwrap();
        assert_eq!((copy.len(), draft.as_str()), (4, ""));
        assert!(matches!(copy.last(), Some(Record::Lua { .. })));
        assert!(copy.iter().all(|r| !matches!(r, Record::Usage { .. })));
        assert!(fork_records(&records, 0).is_err());
        assert!(fork_records(&records, 9).is_err());
    }
}
