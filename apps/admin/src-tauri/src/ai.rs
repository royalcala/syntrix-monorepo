use async_trait::async_trait;
use syntrix_ai::chat::ToolExecutor;
use syntrix_ai::tool::ToolCall;
use syntrix_schema::build_registry;
use syntrix_logging::{LogHandle, LogQuery};

pub struct AiToolExecutor {
    pub log_handle: LogHandle,
}

#[async_trait]
impl ToolExecutor for AiToolExecutor {
    async fn execute_tool(&self, _org_id: &str, tool_call: &ToolCall) -> Result<String, String> {
        let name = &tool_call.function.name;
        let args: serde_json::Value =
            serde_json::from_str(&tool_call.function.arguments).unwrap_or_default();

        match name.as_str() {
            "query_entity_advanced" => Ok(r#"{"note":"Entity queries require the Syntrix client app."}"#.into()),
            "search_entity" => Ok(r#"{"note":"Search requires the Syntrix client app."}"#.into()),
            "get_schema" => exec_get_schema().await,
            "audit_query" => Ok(r#"{"note":"Audit queries require the Syntrix client app."}"#.into()),
            "query_logs" => exec_query_logs(self, &args).await,
            "summarize_logs" => exec_summarize_logs(self, &args).await,
            other => Err(format!("Unknown tool: {}", other)),
        }
    }
}

async fn exec_get_schema() -> Result<String, String> {
    let registry = build_registry();
    let json = registry.export_json();
    serde_json::to_string(&json).map_err(|e| e.to_string())
}

async fn exec_query_logs(
    exec: &AiToolExecutor,
    args: &serde_json::Value,
) -> Result<String, String> {
    let query = LogQuery {
        level: args["level"].as_str().map(|s| s.to_string()),
        search: args["text"].as_str().map(|s| s.to_string()),
        limit: args["limit"].as_u64().map(|n| n as usize),
        ..Default::default()
    };
    let results = syntrix_logging::query_logs_impl(&exec.log_handle, &query);
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

async fn exec_summarize_logs(
    exec: &AiToolExecutor,
    args: &serde_json::Value,
) -> Result<String, String> {
    let window_secs = args["window_secs"].as_u64().unwrap_or(3600);
    let summary = syntrix_logging::summarize_logs_impl(
        &exec.log_handle,
        std::time::Duration::from_secs(window_secs),
    );
    serde_json::to_string(&summary).map_err(|e| e.to_string())
}
