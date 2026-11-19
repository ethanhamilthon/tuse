use crate::prompt;
use crate::record::Record;
use crate::session::Session;

impl Session {
    /// The whole session as plain text: system prompt, messages, thinking, tool calls and outputs.
    pub fn trace(&self) -> String {
        let config = self.config();
        let st = self.state.lock().unwrap();
        let system = prompt::system_prompt(&config, st.bounds.drop > 0);
        let mut parts = vec![format!("## system\n{system}")];
        for r in &st.records {
            match r {
                Record::User { text, images, .. } => {
                    let extra = if images.is_empty() {
                        String::new()
                    } else {
                        format!("\n[{} image(s)]", images.len())
                    };
                    parts.push(format!("## user\n{text}{extra}"));
                }
                Record::Assistant {
                    text, reasoning, ..
                } => {
                    if !reasoning.trim().is_empty() {
                        parts.push(format!("## thinking\n{}", reasoning.trim_end()));
                    }
                    if !text.trim().is_empty() {
                        parts.push(format!("## assistant\n{}", text.trim_end()));
                    }
                }
                Record::Tool {
                    command,
                    output,
                    code,
                    ..
                } => {
                    let code = code.map_or("none".to_string(), |c| c.to_string());
                    parts.push(format!("## tool call\n$ {command}"));
                    parts.push(format!(
                        "## tool output (exit {code})\n{}",
                        output.trim_end()
                    ));
                }
                Record::Notice { text } => parts.push(format!("## notice\n{text}")),
                Record::Error { text } => parts.push(format!("## error\n{text}")),
                _ => {}
            }
        }
        parts.join("\n\n")
    }
}
