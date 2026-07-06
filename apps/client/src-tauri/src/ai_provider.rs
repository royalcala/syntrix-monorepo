use std::sync::atomic::{AtomicU64, Ordering};

use syntrix_ai::provider::{ChatMessage, ModelProvider, ProviderChunk, ProviderResponse, ToolCall, ToolDefinition};

pub struct OllamaProvider {
    base_url: String,
    client: reqwest::blocking::Client,
    start_time: AtomicU64,
}

impl OllamaProvider {
    pub fn new(ollama_url: Option<String>) -> Self {
        let base_url = ollama_url
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let base_url = format!("{}/v1", base_url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            base_url,
            client,
            start_time: AtomicU64::new(now),
        }
    }

    fn build_request_body(
        model: &str,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        stream: bool,
    ) -> serde_json::Value {
        let mut request_messages: Vec<serde_json::Value> = vec![serde_json::json!({
            "role": "system",
            "content": system
        })];

        for msg in messages {
            request_messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        let mut body = serde_json::json!({
            "model": model,
            "messages": request_messages,
            "stream": stream,
        });

        if !tools.is_empty() {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(tool_defs);
        }

        body
    }

    fn parse_response(response: serde_json::Value) -> Result<ProviderResponse, String> {
        let choices = response["choices"]
            .as_array()
            .ok_or_else(|| format!("no choices in response: {}", response))?;

        if choices.is_empty() {
            return Ok(ProviderResponse::new(String::new()));
        }

        let message = &choices[0]["message"];
        let content = message["content"].as_str().unwrap_or("").to_string();

        let tool_calls = message["tool_calls"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .filter_map(|tc| {
                        let id = tc["id"].as_str().unwrap_or("call_unknown").to_string();
                        let name = tc["function"]["name"].as_str()?.to_string();
                        let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
                        let args: serde_json::Value =
                            serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
                        Some(ToolCall { id, name, args })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(ProviderResponse::with_tool_calls(content, tool_calls))
    }

    pub fn uptime(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now - self.start_time.load(Ordering::Relaxed)
    }

    fn check_health(&self) -> Result<String, String> {
        let url = format!("{}/../api/tags", self.base_url.trim_end_matches("/v1"));
        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| format!("ollama health check failed: {e}"))?;
        if resp.status().is_success() {
            Ok("ok".to_string())
        } else {
            Err(format!("ollama returned status {}", resp.status()))
        }
    }
}

impl ModelProvider for OllamaProvider {
    fn chat(
        &self,
        model: &str,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse, String> {
        let body = Self::build_request_body(model, system, messages, tools, false);
        let url = format!("{}/chat/completions", self.base_url);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("ollama request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("ollama error {status}: {text}"));
        }

        let json: serde_json::Value = resp
            .json()
            .map_err(|e| format!("failed to parse ollama response: {e}"))?;

        Self::parse_response(json)
    }

    fn chat_stream(
        &self,
        model: &str,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<Box<dyn Iterator<Item = Result<ProviderChunk, String>> + Send>, String> {
        let body = Self::build_request_body(model, system, messages, tools, true);
        let url = format!("{}/chat/completions", self.base_url);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("ollama stream request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("ollama stream error {status}: {text}"));
        }

        let text = resp
            .text()
            .map_err(|e| format!("failed to read stream body: {e}"))?;

        let chunks: Vec<Result<ProviderChunk, String>> = text
            .lines()
            .filter(|line| line.starts_with("data: ") && *line != "data: [DONE]")
            .filter_map(|line| {
                let json_str = line.strip_prefix("data: ")?;
                let json: serde_json::Value = serde_json::from_str(json_str).ok()?;
                let choice = json["choices"].as_array()?.first()?;
                let delta = &choice["delta"];
                let content = delta["content"].as_str().unwrap_or("").to_string();
                let finish_reason = choice["finish_reason"].as_str();

                let tool_calls = delta["tool_calls"]
                    .as_array()
                    .map(|calls| {
                        calls
                            .iter()
                            .filter_map(|tc| {
                                let id = tc["id"].as_str().unwrap_or("call_unknown").to_string();
                                let name =
                                    tc["function"]["name"].as_str()?.to_string();
                                let args_str =
                                    tc["function"]["arguments"].as_str().unwrap_or("{}");
                                let args: serde_json::Value = serde_json::from_str(args_str)
                                    .unwrap_or(serde_json::json!({}));
                                Some(ToolCall { id, name, args })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                Some(Ok(ProviderChunk {
                    delta: content,
                    tool_calls,
                    done: finish_reason == Some("stop") || finish_reason == Some("tool_calls"),
                }))
            })
            .collect();

        Ok(Box::new(chunks.into_iter()))
    }

    fn health(&self) -> Result<String, String> {
        self.check_health()
    }
}
