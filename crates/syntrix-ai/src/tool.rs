use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "tool_call_id")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "token")]
    Token { content: String },
    #[serde(rename = "tool_call")]
    ToolCall { name: String, args: serde_json::Value },
    #[serde(rename = "tool_result")]
    ToolResult { name: String, result: String },
    #[serde(rename = "done")]
    Done { message: String },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.groq.com/openai/v1".into(),
            model: "gpt-oss-20b".into(),
            api_key: None,
        }
    }
}

pub fn default_tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "query_entity_advanced".into(),
            description: "Query an ERP entity with optional filtering, sorting, and pagination. Entities: customers, suppliers, products, invoices, orders, payroll.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "entity": {
                        "type": "string",
                        "enum": ["customers", "suppliers", "products", "invoices", "orders", "payroll"],
                        "description": "Entity type to query"
                    },
                    "filter_field": {
                        "type": "string",
                        "description": "Field name to filter on"
                    },
                    "filter_value": {
                        "type": "string",
                        "description": "Value to filter by"
                    },
                    "sort": {
                        "type": "object",
                        "properties": {
                            "field": { "type": "string" },
                            "direction": { "type": "string", "enum": ["asc", "desc"] }
                        },
                        "description": "Sort configuration"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Max results (default 50, max 200)"
                    }
                },
                "required": ["entity"]
            }),
        },
        ToolDef {
            name: "search_entity".into(),
            description: "Full-text search across entities. Uses Tantivy for fuzzy matching.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search text"
                    },
                    "entities": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Entity types to search (default: all)"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Max results (default 10, max 50)"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "get_schema".into(),
            description: "Get the full entity schema definitions including field types, relations, and indexes.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDef {
            name: "audit_query".into(),
            description: "Query the audit log for change history on entities.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "entity": {
                        "type": "string",
                        "description": "Filter by entity type"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Max results (default 50)"
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "query_logs".into(),
            description: "Query application logs for diagnostics.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "level": {
                        "type": "string",
                        "enum": ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"],
                        "description": "Filter by log level"
                    },
                    "text": {
                        "type": "string",
                        "description": "Search text in log messages"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Max results (default 50)"
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "summarize_logs".into(),
            description: "Get a summary digest of recent application logs including error counts and top spans.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "window_secs": {
                        "type": "number",
                        "description": "Time window in seconds (default 3600)"
                    }
                },
                "required": []
            }),
        },
    ]
}
