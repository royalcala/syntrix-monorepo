use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct SmsRequest {
    pub to: String,
    pub message: String,
}

pub async fn send(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<SmsRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    // TODO: Integrate with Twilio
    Ok(Json(serde_json::json!({"status": "sent", "note": "Twilio integration pending"})))
}
