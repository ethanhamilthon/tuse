use crate::types::{Delta, Reply, ToolCall, Usage};
use serde_json::Value;

/// Folds Chat Completions stream chunks into a `Reply`.
#[derive(Default)]
pub struct Accumulator {
    pub reply: Reply,
    args: Vec<crate::args::ArgStream>,
}

impl Accumulator {
    pub fn chunk(&mut self, v: &Value, on_delta: &mut dyn FnMut(Delta)) {
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            self.reply.usage = Some(usage(u));
        }
        let Some(choice) = v.get("choices").and_then(|c| c.get(0)) else {
            return;
        };
        if let Some(f) = choice.get("finish_reason").and_then(Value::as_str) {
            self.reply.finish = f.to_string();
        }
        let delta = choice.get("delta").or_else(|| choice.get("message"));
        let Some(delta) = delta else { return };
        for key in ["reasoning_content", "reasoning"] {
            if let Some(t) = delta.get(key).and_then(Value::as_str) {
                self.reply.reasoning.push_str(t);
                on_delta(Delta::Reasoning(t));
            }
        }
        if let Some(t) = delta.get("content").and_then(Value::as_str) {
            self.reply.text.push_str(t);
            on_delta(Delta::Text(t));
        }
        for tc in delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.tool_call(tc, on_delta);
        }
    }

    fn tool_call(&mut self, tc: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let calls = &mut self.reply.tool_calls;
        let index = tc.get("index").and_then(Value::as_u64).map(|i| i as usize);
        let id = tc.get("id").and_then(Value::as_str).unwrap_or("");
        let slot = match index {
            Some(i) => i,
            None if !id.is_empty() => calls.len(),
            None => calls.len().saturating_sub(1),
        };
        while calls.len() <= slot {
            self.args.push(Default::default());
            calls.push(ToolCall::default());
        }
        let call = &mut calls[slot];
        if !id.is_empty() {
            call.id = id.to_string();
        }
        let first = call.name.is_empty();
        if let Some(f) = tc.get("function") {
            if let Some(n) = f.get("name").and_then(Value::as_str) {
                call.name.push_str(n);
            }
            if let Some(a) = f.get("arguments").and_then(Value::as_str) {
                call.arguments.push_str(a);
            }
            if !call.name.is_empty() {
                let chunk = if first {
                    call.arguments.as_str()
                } else {
                    f.get("arguments").and_then(Value::as_str).unwrap_or("")
                };
                let (code, title) = self.args[slot].feed(chunk);
                if first || !code.is_empty() || title.is_some() {
                    on_delta(Delta::Tool {
                        index: slot,
                        id: &call.id,
                        name: &call.name,
                        code: &code,
                        title: title.as_deref(),
                    });
                }
            }
        }
    }

    pub fn finish(mut self) -> Reply {
        self.reply.tool_calls.retain(|c| !c.name.is_empty());
        for (i, c) in self.reply.tool_calls.iter_mut().enumerate() {
            if c.id.is_empty() {
                c.id = format!("call_{i}");
            }
        }
        self.reply
    }
}

fn usage(u: &Value) -> Usage {
    let n = |v: Option<&Value>| v.and_then(Value::as_u64).unwrap_or(0);
    let details = u.get("prompt_tokens_details");
    let d = |k: &str| n(details.and_then(|d| d.get(k)));
    Usage {
        input: n(u.get("prompt_tokens")),
        output: n(u.get("completion_tokens")),
        cached: d("cached_tokens"),
        cache_write: d("cache_write_tokens").max(d("cached_creation_tokens")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn folds_tool_calls_and_usage() {
        let mut acc = Accumulator::default();
        let mut text = String::new();
        let mut on = |d: Delta| {
            if let Delta::Text(t) = d {
                text.push_str(t)
            }
        };
        let chunks = [
            json!({"choices":[{"delta":{"content":"Hi"}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"a","function":{"name":"python","arguments":"{\"com"}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"mand\":\"ls\"}"}}]}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
            json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":3,"prompt_tokens_details":{"cached_tokens":4}}}),
        ];
        for c in &chunks {
            acc.chunk(c, &mut on);
        }
        let r = acc.finish();
        assert_eq!(text, "Hi");
        assert_eq!(r.tool_calls[0].arguments, "{\"command\":\"ls\"}");
        assert_eq!(r.finish, "tool_calls");
        assert_eq!(r.usage.unwrap().cached, 4);
    }
}
