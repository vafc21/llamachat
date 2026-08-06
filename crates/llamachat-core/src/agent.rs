use crate::tools::{ToolRegistry, ToolRequest};
use anyhow::Result;

pub struct Agent {
    pub registry: ToolRegistry,
    pub model: String,
    pub base_url: String,
    pub max_tool_rounds: usize,
}

impl Agent {
    pub fn new(registry: ToolRegistry, model: String) -> Self {
        Agent { registry, model, base_url: "http://127.0.0.1:11434".into(), max_tool_rounds: 5 }
    }

    pub async fn run(&self, user_message: &str) -> Result<String> {
        // Reset web-context safety flag at turn start. Each turn gets a fresh
        // slate: web content from a previous turn does not carry over.
        self.registry.reset_web_context();

        let system = self.registry.system_prompt();
        let mut messages: Vec<serde_json::Value> = vec![
            serde_json::json!({"role": "system", "content": system}),
            serde_json::json!({"role": "user", "content": user_message}),
        ];
        for _ in 0..self.max_tool_rounds {
            let response = self.call_model(&messages).await?;
            if let Some(tr) = self.extract_tool_call(&response) {
                let result = self.registry.execute(&tr);
                messages.push(serde_json::json!({"role": "assistant", "content": response}));

                let text = format!(
                    "Tool result for {}: {}",
                    tr.name,
                    result.output.as_deref().unwrap_or(result.error.as_deref().unwrap_or("no output"))
                );

                // A tool that produced an image (a screenshot, say) hands back a
                // path in `media`. Previously that path was pasted in as plain
                // text and the pixels were thrown away, so the model was asked
                // to reason about something it could not see. Encode it and send
                // it properly — but only to a model that can actually look.
                messages.push(match result.media.as_deref() {
                    Some(path) if crate::vision::is_vision_model(&self.model) => {
                        match crate::vision::encode_file(path) {
                            Ok(img) => crate::vision::user_message_with_images(
                                &format!("{text}\n[image attached: {}]", img.summary()),
                                std::slice::from_ref(&img),
                            ),
                            // Be explicit rather than silently continuing blind.
                            Err(e) => serde_json::json!({
                                "role": "user",
                                "content": format!("{text}\n[image could not be attached: {e}]"),
                            }),
                        }
                    }
                    Some(_) => serde_json::json!({
                        "role": "user",
                        "content": format!(
                            "{text}\n[an image was captured, but {} cannot accept images — \
                             load a vision model to look at it]",
                            self.model
                        ),
                    }),
                    None => serde_json::json!({"role": "user", "content": text}),
                });
                continue;
            }
            return Ok(response);
        }
        Ok("Max tool rounds reached.".into())
    }

    async fn call_model(&self, messages: &[serde_json::Value]) -> Result<String> {
        let resp = reqwest::Client::new()
            .post(format!("{}/api/chat", self.base_url))
            .json(&serde_json::json!({"model": &self.model, "messages": messages, "stream": false}))
            .timeout(std::time::Duration::from_secs(120))
            .send().await?;
        let body: serde_json::Value = resp.json().await?;
        Ok(body["message"]["content"].as_str().unwrap_or("").to_string())
    }

    fn extract_tool_call(&self, text: &str) -> Option<ToolRequest> {
        for prefix in &["{\"tool\"", "```json\n{\"tool\"", "```\n{\"tool\""] {
            if let Some(pos) = text.find(prefix) {
                let start = if prefix.contains("```") { pos + prefix.len() - "{\"tool\"".len() } else { pos };
                let slice = &text[start..];
                let mut depth = 0i32;
                let mut end = 0;
                for (i, ch) in slice.char_indices() {
                    if ch == '{' { depth += 1; }
                    if ch == '}' { depth -= 1; if depth == 0 { end = i + 1; break; } }
                }
                if end > 0 {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&slice[..end]) {
                        if let Some(name) = val.get("tool").and_then(|t| t.as_str()) {
                            return Some(ToolRequest {
                                name: name.to_string(),
                                args: val.get("args").cloned().unwrap_or(serde_json::json!({})),
                            });
                        }
                    }
                }
            }
        }
        None
    }
}
