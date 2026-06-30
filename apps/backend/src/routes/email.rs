use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct EmailRequest {
    pub to: String,
    pub subject: String,
    pub body: String,
}

pub async fn send(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<EmailRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    // TODO: Integrate with Resend API
    Ok(Json(serde_json::json!({"status": "sent", "note": "Resend integration pending"})))
}
