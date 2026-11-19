use crate::bash;
use crate::event::Event;
use crate::id;
use crate::record::Record;
use crate::session::Session;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tuse_api::ToolCall;

impl Session {
    /// Runs one tool call and records its result. False when the turn went stale.
    pub(crate) fn tool(
        &self,
        generation: u64,
        cancel: &AtomicBool,
        call: &ToolCall,
        config: &tuse_config::Config,
    ) -> bool {
        let command = bash::command_of(&call.arguments);
        let timeout = serde_json::from_str::<serde_json::Value>(&call.arguments)
            .ok()
            .and_then(|v| v["timeout"].as_u64())
            .unwrap_or(config.settings.bash_timeout);
        self.emit(Event::ToolStart {
            id: call.id.clone(),
            command: command.clone(),
        });
        self.hook(
            "tool_start",
            vec![("TUSE_TOOL_COMMAND".into(), command.clone())],
        );
        let out = if call.name == "bash" {
            let cwd = PathBuf::from(&self.cwd);
            bash::run(
                &command,
                &cwd,
                &self.env(),
                Duration::from_secs(timeout.max(1)),
                config.settings.max_bash_output,
                cancel,
            )
        } else {
            bash::Output {
                text: format!("unknown tool `{}`; the only tool is `bash`", call.name),
                code: None,
            }
        };
        let Some(mut st) = self.lock_live(generation) else {
            return false;
        };
        st.calls += 1;
        let r = Record::Tool {
            id: call.id.clone(),
            command: command.clone(),
            output: out.text.clone(),
            code: out.code,
            at: id::now(),
        };
        self.push(&mut st, r);
        drop(st);
        self.emit(Event::ToolEnd {
            id: call.id.clone(),
            output: out.text,
            code: out.code,
        });
        let code = out.code.map(|c| c.to_string()).unwrap_or_default();
        self.hook(
            "tool_end",
            vec![
                ("TUSE_TOOL_COMMAND".into(), command),
                ("TUSE_TOOL_EXIT".into(), code),
            ],
        );
        true
    }
}
