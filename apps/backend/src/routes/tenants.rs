use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;
use crate::auth;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub org_id: String,
    pub name: Option<String>,
    pub public_key_hex: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub tenant_id: String,
    pub api_key: String,
    pub jwt: String,
    pub wallet_id: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let api_key = uuid::Uuid::new_v4().to_string();
    let api_key_hash = sha256_hash(&api_key);

    let wallet_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO wallets (id) VALUES ($1)")
        .bind(wallet_id)
        .execute(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let tenant_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO tenants (id, org_id, name, public_key_hex, api_key_hash, wallet_id) VALUES ($1, $2, $3, $4, $5, $6)"
    )
        .bind(tenant_id)
        .bind(&req.org_id)
        .bind(&req.name)
        .bind(&req.public_key_hex)
        .bind(&api_key_hash)
        .bind(wallet_id)
        .execute(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let jwt = auth::create_jwt(
        &state.jwt_secret,
        &tenant_id.to_string(),
        Some(tenant_id.to_string()),
        "client",
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;

    Ok(Json(RegisterResponse {
        tenant_id: tenant_id.to_string(),
        api_key,
        jwt,
        wallet_id: wallet_id.to_string(),
    }))
}

fn sha256_hash(input: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}
