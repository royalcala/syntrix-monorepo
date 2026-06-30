use std::sync::Arc;

use async_trait::async_trait;
use syntrix_ai::chat::ToolExecutor;
use syntrix_ai::tool::ToolCall;
use syntrix_schema::build_registry;
use syntrix_logging::{LogHandle, LogQuery};

use crate::indexes::RelationalEngine;

pub struct AiToolExecutor {
    pub indexer: Arc<RelationalEngine>,
    pub log_handle: LogHandle,
}

#[async_trait]
impl ToolExecutor for AiToolExecutor {
    async fn execute_tool(&self, org_id: &str, tool_call: &ToolCall) -> Result<String, String> {
        let name = &tool_call.function.name;
        let args: serde_json::Value =
            serde_json::from_str(&tool_call.function.arguments).unwrap_or_default();

        match name.as_str() {
            "query_entity_advanced" => exec_query_entity(self, org_id, &args).await,
            "search_entity" => exec_search_entity(self, org_id, &args).await,
            "get_schema" => exec_get_schema().await,
            "audit_query" => exec_audit_query(&args).await,
            "query_logs" => exec_query_logs(self, &args).await,
            "summarize_logs" => exec_summarize_logs(self, &args).await,
            other => Err(format!("Unknown tool: {}", other)),
        }
    }
}

async fn exec_query_entity(
    exec: &AiToolExecutor,
    org_id: &str,
    args: &serde_json::Value,
) -> Result<String, String> {
    let entity = args["entity"].as_str().ok_or("missing 'entity' parameter")?;
    let filter_field = args["filter_field"].as_str().map(|s| s.to_string());
    let filter_value = args["filter_value"].as_str().map(|s| s.to_string());
    let limit = args["limit"].as_u64().map(|n| n as usize).unwrap_or(50).min(200);

    let mut filters = vec![];
    if let (Some(field), Some(value)) = (&filter_field, &filter_value) {
        filters.push(crate::indexes::QueryFilter {
            field: field.clone(),
            value: value.clone(),
        });
    }

    let options = crate::indexes::QueryOptions {
        filters,
        sort: Some("id".into()),
        limit: Some(limit),
        offset: None,
    };

    let results = exec.indexer.query(org_id, entity, &options).map_err(|e| e.to_string())?;
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

async fn exec_search_entity(
    exec: &AiToolExecutor,
    org_id: &str,
    args: &serde_json::Value,
) -> Result<String, String> {
    let query = args["query"].as_str().ok_or("missing 'query' parameter")?;
    let entities = args["entities"].as_array().map(|arr| {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>()
    });
    let limit = args["limit"].as_u64().map(|n| n as usize).unwrap_or(10).min(50);

    let results = exec.indexer.search_engine.search(org_id, query, entities, limit)
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

async fn exec_get_schema() -> Result<String, String> {
    let registry = build_registry();
    let json = registry.export_json();
    serde_json::to_string(&json).map_err(|e| e.to_string())
}

async fn exec_audit_query(args: &serde_json::Value) -> Result<String, String> {
    let entity = args["entity"].as_str().map(|s| s.to_string());
    let limit = args["limit"].as_u64().map(|n| n as usize).unwrap_or(50);
    Ok(serde_json::json!({
        "note": "Audit query requires full AppState. Use the audit_query command directly for detailed results.",
        "entity_filter": entity,
        "limit": limit,
    }).to_string())
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
