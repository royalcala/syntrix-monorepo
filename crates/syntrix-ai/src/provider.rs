use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

impl ProviderResponse {
    pub fn new(content: String) -> Self {
        Self {
            content,
            tool_calls: vec![],
        }
    }

    pub fn with_tool_calls(content: String, tool_calls: Vec<ToolCall>) -> Self {
        Self { content, tool_calls }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderChunk {
    pub delta: String,
    pub tool_calls: Vec<ToolCall>,
    pub done: bool,
}

pub trait ModelProvider: Send + Sync {
    fn chat(
        &self,
        model: &str,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse, String>;

    fn chat_stream(
        &self,
        model: &str,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<Box<dyn Iterator<Item = Result<ProviderChunk, String>> + Send>, String>;

    fn health(&self) -> Result<String, String>;
}

#[cfg(test)]
pub mod test_utils {
    use super::*;

    pub struct MockProvider {
        pub response: String,
        pub tool_calls: Vec<ToolCall>,
        pub health_ok: bool,
    }

    impl MockProvider {
        pub fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
                tool_calls: vec![],
                health_ok: true,
            }
        }

        pub fn with_tool_calls(response: &str, tool_calls: Vec<ToolCall>) -> Self {
            Self {
                response: response.to_string(),
                tool_calls,
                health_ok: true,
            }
        }
    }

    impl ModelProvider for MockProvider {
        fn chat(
            &self,
            _model: &str,
            _system: &str,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
        ) -> Result<ProviderResponse, String> {
            Ok(ProviderResponse {
                content: self.response.clone(),
                tool_calls: self.tool_calls.clone(),
            })
        }

        fn chat_stream(
            &self,
            _model: &str,
            _system: &str,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
        ) -> Result<Box<dyn Iterator<Item = Result<ProviderChunk, String>> + Send>, String> {
            let chunks: Vec<Result<ProviderChunk, String>> = vec![
                Ok(ProviderChunk {
                    delta: self.response.clone(),
                    tool_calls: vec![],
                    done: false,
                }),
                Ok(ProviderChunk {
                    delta: String::new(),
                    tool_calls: vec![],
                    done: true,
                }),
            ];
            Ok(Box::new(chunks.into_iter()))
        }

        fn health(&self) -> Result<String, String> {
            if self.health_ok {
                Ok("ok".to_string())
            } else {
                Err("unhealthy".to_string())
            }
        }
    }
}

pub struct StatefulMockProvider {
    response: String,
    tool_calls_on_first: Vec<ToolCall>,
    call_count: std::sync::atomic::AtomicU64,
}

impl StatefulMockProvider {
    pub fn new() -> Self {
        Self {
            response: String::new(),
            tool_calls_on_first: vec![],
            call_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn with_tool_call(mut self, id: &str, name: &str, args: serde_json::Value) -> Self {
        self.tool_calls_on_first.push(ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            args,
        });
        self
    }
}

impl ModelProvider for StatefulMockProvider {
    fn chat(
        &self,
        _model: &str,
        _system: &str,
        _messages: &[ChatMessage],
        _tools: &[ToolDefinition],
    ) -> Result<ProviderResponse, String> {
        let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count == 0 && !self.tool_calls_on_first.is_empty() {
            Ok(ProviderResponse::with_tool_calls(self.response.clone(), self.tool_calls_on_first.clone()))
        } else {
            Ok(ProviderResponse::new("Resultado final".to_string()))
        }
    }

    fn chat_stream(
        &self,
        _model: &str,
        _system: &str,
        _messages: &[ChatMessage],
        _tools: &[ToolDefinition],
    ) -> Result<Box<dyn Iterator<Item = Result<ProviderChunk, String>> + Send>, String> {
        Ok(Box::new(std::iter::empty()))
    }

    fn health(&self) -> Result<String, String> {
        Ok("ok".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::MockProvider;

    #[test]
    fn test_mock_provider_returns_response() {
        let provider = MockProvider::new("hello");
        let response = provider
            .chat("test", "system", &[], &[])
            .unwrap();
        assert_eq!(response.content, "hello");
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn test_mock_provider_with_tool_calls() {
        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            name: "query_entity".to_string(),
            args: serde_json::json!({"sql": "SELECT * FROM customers"}),
        }];
        let provider = MockProvider::with_tool_calls("", tool_calls.clone());
        let response = provider
            .chat("test", "system", &[], &[])
            .unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "query_entity");
    }

    #[test]
    fn test_mock_provider_health() {
        let provider = MockProvider::new("ok");
        assert!(provider.health().is_ok());

        let mut unhealthy = MockProvider::new("ok");
        unhealthy.health_ok = false;
        assert!(unhealthy.health().is_err());
    }

    #[test]
    fn test_provider_response_builders() {
        let r1 = ProviderResponse::new("hello".to_string());
        assert!(r1.tool_calls.is_empty());

        let tc = vec![ToolCall {
            id: "c1".to_string(),
            name: "test".to_string(),
            args: serde_json::json!({}),
        }];
        let r2 = ProviderResponse::with_tool_calls("".to_string(), tc);
        assert_eq!(r2.tool_calls.len(), 1);
    }
}
