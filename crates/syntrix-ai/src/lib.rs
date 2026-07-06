pub mod bench;
pub mod provider;
pub mod router;
pub mod tools;

use serde::{Deserialize, Serialize};
use tools::AiContext;

pub use provider::{ChatMessage, ModelProvider, ProviderResponse, ToolCall, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatRequest {
    pub org_id: String,
    pub messages: Vec<AiMessage>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatResponse {
    pub reply: String,
    pub tool_calls: Vec<ToolCallRecord>,
    pub status: AiStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AiStatus {
    Cached,
    Generated,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatusInfo {
    pub models_available: Vec<String>,
    pub default_model: String,
    pub health: String,
    pub uptime_seconds: u64,
}

fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "get_schema".to_string(),
            description: "Obtener metadata del schema: entidades, columnas, tipos, relaciones padre-hijo".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "entity": {
                        "type": "string",
                        "description": "Nombre de entidad opcional (ej: invoices, customers). Si se omite, lista todas."
                    }
                }
            }),
        },
        ToolDefinition {
            name: "query_entity".to_string(),
            description: "Ejecutar SQL SELECT/WITH contra la base de datos local. Solo lecturas.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "sql": {"type": "string", "description": "Sentencia SQL SELECT o WITH"},
                    "params": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["sql"]
            }),
        },
        ToolDefinition {
            name: "search_entity".to_string(),
            description: "Buscar texto completo en entidades indexadas".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Texto a buscar"},
                    "entities": {"type": "array", "items": {"type": "string"}},
                    "limit": {"type": "integer"}
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "save_view".to_string(),
            description: "Guardar una ViewDefinition como vista reutilizable".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "view": {"type": "object", "description": "ViewDefinition con sql, entity, components, meta"}
                },
                "required": ["view"]
            }),
        },
        ToolDefinition {
            name: "list_views".to_string(),
            description: "Listar ViewDefinitions guardadas, opcionalmente filtradas por tags".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tags": {"type": "array", "items": {"type": "string"}}
                }
            }),
        },
    ]
}

fn build_system_prompt() -> Result<String, String> {
    let schema_info = tools::get_schema_impl(None)
        .map(|entities| serde_json::to_value(entities).unwrap_or_default())
        .unwrap_or_default();

    Ok(format!(
        r#"Eres un asistente de IA para Syntrix P2P, una plataforma de colaboración local-first.
Tienes acceso a los siguientes tools para consultar la base de datos local:

1. get_schema(entity) — Obtener metadata de entidades, columnas, tipos y relaciones
2. query_entity(sql, params) — Ejecutar SQL SELECT/WITH contra la base de datos local
3. search_entity(query, entities, limit) — Búsqueda de texto completo
4. save_view(view) — Guardar una vista para uso futuro
5. list_views(tags) — Listar vistas guardadas

Schema disponible en esta organización:
{}

Importante:
- Siempre usa get_schema primero para conocer las columnas disponibles antes de generar SQL.
- Si el usuario pide una vista, genera el SQL, ejecútalo con query_entity para validar, y luego guarda con save_view.
- Responde en español.
- Las fechas están en formato TEXT (ISO 8601).
- Los IDs de entidades relacionadas terminan en '_id'.
- NUNCA generes DELETE, UPDATE, INSERT, DROP, ALTER — solo SELECT/WITH."#,
        schema_info
    ))
}

fn execute_tool(
    ctx: &dyn AiContext,
    org_id: &str,
    name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match name {
        "get_schema" => {
            let entity = args.get("entity").and_then(|v| v.as_str());
            let schema = tools::get_schema_impl(entity)?;
            serde_json::to_value(schema).map_err(|e| e.to_string())
        }
        "query_entity" => {
            let sql = args["sql"]
                .as_str()
                .ok_or_else(|| "missing sql argument".to_string())?;
            let params: Vec<String> = args["params"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let result = tools::query_entity_tool(ctx, org_id, sql, &params)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "search_entity" => {
            let query = args["query"]
                .as_str()
                .ok_or_else(|| "missing query argument".to_string())?;
            let entities = args["entities"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                });
            let limit = args["limit"].as_u64().unwrap_or(20) as usize;
            let result =
                tools::search_entity_tool(ctx, org_id, query, entities, limit)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "save_view" => {
            let view_val = args
                .get("view")
                .ok_or_else(|| "missing view argument".to_string())?;
            let view: tools::ViewDefinition =
                serde_json::from_value(view_val.clone()).map_err(|e| format!("invalid view: {e}"))?;
            tools::save_view_tool(ctx, &view)?;
            Ok(serde_json::json!({"ok": true, "id": view.id}))
        }
        "list_views" => {
            let tags = args["tags"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                });
            let views = tools::list_views_tool(ctx, org_id, tags.as_deref())?;
            serde_json::to_value(views).map_err(|e| e.to_string())
        }
        _ => Err(format!("unknown tool: {}", name)),
    }
}

pub fn ai_chat_impl(
    ctx: &dyn AiContext,
    provider: &dyn ModelProvider,
    router: &dyn router::ModelRouter,
    req: &AiChatRequest,
) -> Result<AiChatResponse, String> {
    let user_msg = req
        .messages
        .iter()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let task_type = router::TaskType::classify(&user_msg);
    let model_selection = router.select(task_type);
    let model = req.model.as_deref().unwrap_or(model_selection.primary);
    let tools = tool_definitions();

    let system_prompt = build_system_prompt()?;

    let chat_messages: Vec<ChatMessage> = req
        .messages
        .iter()
        .map(|m| ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let mut all_messages = chat_messages.clone();
    let mut tool_call_records = Vec::new();
    let mut final_content = String::new();

    for iteration in 0..10 {
        let provider_response =
            provider.chat(model, &system_prompt, &all_messages, &tools)?;

        let has_tool_calls = !provider_response.tool_calls.is_empty();
        final_content.clone_from(&provider_response.content);

        if !has_tool_calls {
            return Ok(AiChatResponse {
                reply: provider_response.content,
                tool_calls: tool_call_records,
                status: AiStatus::Generated,
            });
        }

        let assistant_msg = ChatMessage {
            role: "assistant".to_string(),
            content: provider_response.content.clone(),
        };
        all_messages.push(assistant_msg);

        for tool_call in &provider_response.tool_calls {
            let result = execute_tool(ctx, &req.org_id, &tool_call.name, &tool_call.args);
            let result_value = match &result {
                Ok(v) => v.clone(),
                Err(e) => serde_json::json!({"error": e}),
            };

            tool_call_records.push(ToolCallRecord {
                tool: tool_call.name.clone(),
                args: tool_call.args.clone(),
                result: result_value.clone(),
            });

            let tool_result_msg = ChatMessage {
                role: "tool".to_string(),
                content: serde_json::to_string(&result_value).unwrap_or_default(),
            };
            all_messages.push(tool_result_msg);
        }

        if iteration >= 9 {
            return Ok(AiChatResponse {
                reply: final_content,
                tool_calls: tool_call_records,
                status: AiStatus::Error,
            });
        }
    }

    Err("ai_chat_impl: reached unreachable state".to_string())
}

pub fn ai_status_impl(uptime_seconds: u64) -> AiStatusInfo {
    AiStatusInfo {
        models_available: vec![
            "granite3.2:2b".to_string(),
            "granite4.1:3b".to_string(),
            "qwen2.5-coder:3b".to_string(),
            "deepseek-r1:1.5b".to_string(),
        ],
        default_model: "qwen2.5-coder:3b".to_string(),
        health: "ok".to_string(),
        uptime_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provider::test_utils::MockProvider;
    use tools::{MockAiContext, ViewDefinition, ViewMeta};

    #[test]
    fn test_ai_chat_impl_with_mock_provider() {
        let ctx = MockAiContext::new(true);
        let provider = MockProvider::new("Aquí tienes las facturas de mayo.");
        let router = router::DefaultRouter;
        let req = AiChatRequest {
            org_id: "org-1".to_string(),
            messages: vec![AiMessage {
                role: "user".to_string(),
                content: "muéstrame las facturas de mayo".to_string(),
            }],
            model: None,
        };

        let resp = ai_chat_impl(&ctx, &provider, &router, &req).unwrap();
        assert!(resp.status == AiStatus::Generated);
        assert!(resp.reply.contains("mayo"));
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn test_ai_chat_impl_executes_tool_call() {
        let ctx = MockAiContext::new(true);
        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            name: "get_schema".to_string(),
            args: serde_json::json!({"entity": "customers"}),
        }];
        let provider = MockProvider::with_tool_calls(
            "Primero consulto el schema de customers.",
            tool_calls,
        );
        let router = router::DefaultRouter;
        let req = AiChatRequest {
            org_id: "org-1".to_string(),
            messages: vec![AiMessage {
                role: "user".to_string(),
                content: "dame los clientes".to_string(),
            }],
            model: None,
        };

        let resp = ai_chat_impl(&ctx, &provider, &router, &req).unwrap();
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].tool, "get_schema");
    }

    #[test]
    fn test_ai_chat_impl_multiple_tool_rounds() {
        let ctx = MockAiContext::new(true);
        let provider = MockProvider::with_tool_calls(
            "Procesando...",
            vec![ToolCall {
                id: "call_1".to_string(),
                name: "query_entity".to_string(),
                args: serde_json::json!({"sql": "SELECT * FROM customers"}),
            }],
        );
        let router = router::DefaultRouter;
        let req = AiChatRequest {
            org_id: "org-1".to_string(),
            messages: vec![AiMessage {
                role: "user".to_string(),
                content: "lista de clientes".to_string(),
            }],
            model: None,
        };

        let resp = ai_chat_impl(&ctx, &provider, &router, &req).unwrap();
        assert!(!resp.tool_calls.is_empty());
    }

    #[test]
    fn test_ai_status_returns_models() {
        let status = ai_status_impl(42);
        assert_eq!(status.health, "ok");
        assert_eq!(status.uptime_seconds, 42);
        assert!(status.models_available.len() >= 3);
    }

    #[test]
    fn test_build_system_prompt_includes_schema() {
        let prompt = build_system_prompt().unwrap();
        assert!(prompt.contains("customers"));
        assert!(prompt.contains("invoices"));
        assert!(prompt.contains("SELECT"));
    }

    #[test]
    fn test_tool_definitions_has_five_tools() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 5);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"get_schema"));
        assert!(names.contains(&"query_entity"));
        assert!(names.contains(&"search_entity"));
        assert!(names.contains(&"save_view"));
        assert!(names.contains(&"list_views"));
    }

    #[test]
    fn test_execute_get_schema_tool() {
        let ctx = MockAiContext::new(true);
        let result = execute_tool(
            &ctx,
            "org-1",
            "get_schema",
            &serde_json::json!({"entity": "invoices"}),
        )
        .unwrap();
        let entities: Vec<tools::SchemaEntity> =
            serde_json::from_value(result).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "invoices");
    }

    #[test]
    fn test_execute_query_entity_validates_sql() {
        let ctx = MockAiContext::new(true);
        let result = execute_tool(
            &ctx,
            "org-1",
            "query_entity",
            &serde_json::json!({"sql": "DELETE FROM customers"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_unknown_tool_returns_error() {
        let ctx = MockAiContext::new(true);
        let result = execute_tool(&ctx, "org-1", "nonexistent", &serde_json::json!({}));
        assert!(result.is_err());
    }
}
