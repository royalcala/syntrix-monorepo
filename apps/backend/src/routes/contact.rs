use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct ContactRequest {
    pub name: Option<String>,
    pub email: String,
    pub message: String,
}

pub async fn submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ContactRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    sqlx::query("INSERT INTO contact_inquiries (id, name, email, message) VALUES ($1, $2, $3, $4)")
        .bind(Uuid::new_v4())
        .bind(&req.name)
        .bind(&req.email)
        .bind(&req.message)
        .execute(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(serde_json::json!({"status": "sent"})))
}
