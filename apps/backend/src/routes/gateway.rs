use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<GatewayMessage>,
    pub stream: Option<bool>,
}

#[derive(Deserialize, Serialize)]
pub struct GatewayMessage {
    pub role: String,
    pub content: Option<String>,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub choices: Vec<Choice>,
}

#[derive(Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: GatewayMessage,
}

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let provider_info = crate::auth::get_provider_for_model(&req.model);

    let provider = syntrix_ai::provider::OpenAICompatibleProvider::new(
        provider_info.base_url,
        req.model.clone(),
        Some(provider_info.api_key),
    );

    let tools = syntrix_ai::tool::default_tool_definitions();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let sys_msg = syntrix_ai::tool::ChatMessage {
        role: "system".into(),
        content: Some("You are the Syntrix AI Gateway. You can query ERP data using tools.".into()),
        tool_calls: None,
        tool_call_id: None,
    };

    let mut msgs = vec![sys_msg];
    for m in req.messages {
        msgs.push(syntrix_ai::tool::ChatMessage {
            role: m.role,
            content: m.content,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    let _ = tokio::spawn(async move {
        // Headless executor stub — returns "gateway mode" responses for tool calls
        let executor = crate::routes::gateway::GatewayExecutor;
        syntrix_ai::chat::ai_chat_impl(
            &provider, &executor, "gateway", &msgs, &tools, tx,
        ).await.ok();
    });

    let mut content = String::new();
    while let Some(event) = rx.recv().await {
        if let syntrix_ai::tool::StreamEvent::Token { content: c } = event {
            content.push_str(&c);
        }
    }

    // Meter usage
    let tenant_id = uuid::Uuid::new_v4(); // TODO: extract from claims
    let tokens = content.len() as i64 / 4; // rough estimate
    let cost = tokens * 75 / 1_000_000; // $0.075 per 1M tokens
    sqlx::query(
        "INSERT INTO service_usage_log (id, tenant_id, service, quantity, cost_cents) VALUES ($1, $2, 'ai_gateway', $3, $4)"
    )
        .bind(uuid::Uuid::new_v4())
        .bind(tenant_id)
        .bind(tokens.max(1))
        .bind(cost.max(1))
        .execute(&state.db)
        .await
        .ok();

    Ok(Json(ChatResponse {
        id: uuid::Uuid::new_v4().to_string(),
        object: "chat.completion".into(),
        choices: vec![Choice {
            index: 0,
            message: GatewayMessage {
                role: "assistant".into(),
                content: Some(content),
            },
        }],
    }))
}

pub struct GatewayExecutor;

#[async_trait::async_trait]
impl syntrix_ai::chat::ToolExecutor for GatewayExecutor {
    async fn execute_tool(&self, _org_id: &str, tool_call: &syntrix_ai::tool::ToolCall) -> Result<String, String> {
        Ok(serde_json::json!({"note": "Gateway passthrough mode — tool execution not available at the gateway level. Use the Syntrix desktop app for direct data access."}).to_string())
    }
}


