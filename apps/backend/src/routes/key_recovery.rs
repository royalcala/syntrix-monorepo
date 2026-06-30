use axum::{extract::{State, Json, Query}, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct StoreRequest {
    pub device_label: String,
    pub encrypted_blob: String,
    pub encryption_salt: String,
}

pub async fn store(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StoreRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let blob_bytes = hex::decode(&req.encrypted_blob)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))))?;

    sqlx::query(
        "INSERT INTO key_recovery_blobs (id, tenant_id, device_label, encrypted_blob, encryption_salt) VALUES ($1, $2, $3, $4, $5)"
    )
        .bind(Uuid::new_v4())
        .bind(Uuid::nil()) // TODO: extract tenant from claims
        .bind(&req.device_label)
        .bind(&blob_bytes)
        .bind(&req.encryption_salt)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(serde_json::json!({"status": "stored"})))
}

#[derive(Deserialize)]
pub struct RetrieveParams {
    pub device_label: String,
}

#[derive(Serialize)]
pub struct RetrieveResponse {
    pub encrypted_blob: String,
    pub encryption_salt: String,
}

pub async fn retrieve(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RetrieveParams>,
) -> Result<Json<RetrieveResponse>, (StatusCode, Json<serde_json::Value>)> {
    let blob = sqlx::query_as::<_, (Vec<u8>, String)>(
        "SELECT encrypted_blob, encryption_salt FROM key_recovery_blobs WHERE device_label = $1 ORDER BY created_at DESC LIMIT 1"
    )
        .bind(&params.device_label)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "blob not found"}))))?;

    Ok(Json(RetrieveResponse {
        encrypted_blob: hex::encode(&blob.0),
        encryption_salt: blob.1,
    }))
}

