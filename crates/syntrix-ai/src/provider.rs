use async_trait::async_trait;
use futures_util::StreamExt;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::tool::{ChatMessage, StreamEvent, ToolDef};

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        sender: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<(), String>;
}

pub struct OpenAICompatibleProvider {
    client: reqwest::Client,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl OpenAICompatibleProvider {
    pub fn new(base_url: String, model: String, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            model,
            api_key,
        }
    }
}

fn convert_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    messages.iter().map(|m| {
        let mut obj = serde_json::json!({
            "role": m.role,
            "content": m.content,
        });
        if let Some(ref calls) = m.tool_calls {
            obj["tool_calls"] = serde_json::json!(calls);
        }
        if let Some(ref id) = m.tool_call_id {
            obj["tool_call_id"] = serde_json::json!(id);
        }
        obj
    }).collect()
}

fn convert_tools(tools: &[ToolDef]) -> Vec<serde_json::Value> {
    tools.iter().map(|t| {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            }
        })
    }).collect()
}

#[async_trait]
impl ModelProvider for OpenAICompatibleProvider {
    async fn complete_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        sender: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<(), String> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": convert_messages(messages),
            "stream": true,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(convert_tools(tools));
        }

        let mut req = self.client.post(&url).json(&body);

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().await.map_err(|e| format!("request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, text));
        }

        let stream = response.bytes_stream();
        tokio::pin!(stream);

        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| format!("stream error: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if data == "[DONE]" {
                    continue;
                }

                match serde_json::from_str::<serde_json::Value>(data) {
                    Ok(json) => {
                        if let Some(choices) = json["choices"].as_array() {
                            for choice in choices {
                                let delta = &choice["delta"];
                                if let Some(content) = delta["content"].as_str() {
                                    if !content.is_empty() {
                                        let _ = sender.send(StreamEvent::Token { content: content.to_string() });
                                    }
                                }
                                if let Some(tool_calls) = delta["tool_calls"].as_array() {
                                    for tc in tool_calls {
                                        if let (Some(name), Some(args)) = (
                                            tc["function"]["name"].as_str(),
                                            tc["function"]["arguments"].as_str(),
                                        ) {
                                            if let Ok(parsed_args) = serde_json::from_str::<serde_json::Value>(args) {
                                                let _ = sender.send(StreamEvent::ToolCall {
                                                    name: name.to_string(),
                                                    args: parsed_args,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        tracing::warn!("failed to parse SSE chunk: {}", data);
                    }
                }
            }
        }

        Ok(())
    }
}
