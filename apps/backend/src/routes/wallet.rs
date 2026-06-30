use axum::{extract::{State, Path}, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize)]
pub struct WalletResponse {
    pub id: String,
    pub balance_cents: i64,
    pub reserved_cents: i64,
}

pub async fn get_balance(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::Claims>,
) -> Result<Json<WalletResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let tenant_id = claims.tenant_id.as_ref()
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| (axum::http::StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid tenant"}))))?;

    let wallet = sqlx::query_as::<_, crate::db::models::Wallet>(
        "SELECT w.* FROM wallets w JOIN tenants t ON t.wallet_id = w.id WHERE t.id = $1"
    )
        .bind(tenant_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(WalletResponse {
        id: wallet.id.to_string(),
        balance_cents: wallet.balance_cents,
        reserved_cents: wallet.reserved_cents,
    }))
}

#[derive(Deserialize)]
pub struct TopupRequest {
    pub amount_cents: i64,
    pub idempotency_key: Option<String>,
}

#[derive(Serialize)]
pub struct TopupResponse {
    pub client_secret: String,
}

pub async fn create_topup(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::Claims>,
    Json(req): Json<TopupRequest>,
) -> Result<Json<TopupResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let tenant_id = claims.tenant_id.as_ref()
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| (axum::http::StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid tenant"}))))?;

    let wallet = sqlx::query_as::<_, crate::db::models::Wallet>(
        "SELECT w.* FROM wallets w JOIN tenants t ON t.wallet_id = w.id WHERE t.id = $1"
    )
        .bind(tenant_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e.to_string()}))))?;

    let stripe = stripe::Client::new(&state.stripe_secret);
    let mut intent_params = stripe::CreatePaymentIntent::new(
        stripe::currency::MXN,
        req.amount_cents as i64,
    );
    intent_params.metadata = Some(std::collections::HashMap::from([
        ("wallet_id".to_string(), wallet.id.to_string()),
        ("tenant_id".to_string(), tenant_id.to_string()),
    ]));
    if let Some(ref key) = req.idempotency_key {
        intent_params.idempotency = Some(stripe::IdempotencyKey(key.clone()));
    }

    let intent = stripe::PaymentIntent::create(&stripe, intent_params)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(TopupResponse {
        client_secret: intent.client_secret.unwrap_or_default(),
    }))
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub id: String,
    pub r#type: String,
    pub amount_cents: i64,
    pub service: Option<String>,
    pub created_at: String,
}

pub async fn get_transactions(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::Claims>,
) -> Result<Json<Vec<TransactionResponse>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let tenant_id = claims.tenant_id.as_ref()
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| (axum::http::StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid tenant"}))))?;

    let txs = sqlx::query_as::<_, crate::db::models::WalletTransaction>(
        "SELECT wt.* FROM wallet_transactions wt JOIN wallets w ON wt.wallet_id = w.id
         JOIN tenants t ON t.wallet_id = w.id WHERE t.id = $1 ORDER BY wt.created_at DESC LIMIT 100"
    )
        .bind(tenant_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let res: Vec<TransactionResponse> = txs.into_iter().map(|tx| TransactionResponse {
        id: tx.id.to_string(),
        r#type: tx.r#type,
        amount_cents: tx.amount_cents,
        service: tx.service,
        created_at: tx.created_at.to_rfc3339(),
    }).collect();

    Ok(Json(res))
}

#[derive(Deserialize)]
pub struct StripeWebhookEvent {
    pub id: String,
    pub event_type: Option<String>,
    pub data: serde_json::Value,
}

pub async fn stripe_webhook(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let payload: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))))?;

    let event_type = payload["type"].as_str().unwrap_or("");

    if event_type == "payment_intent.succeeded" {
        let intent = &payload["data"]["object"];
        let wallet_id_str = intent["metadata"]["wallet_id"].as_str()
            .ok_or_else(|| (axum::http::StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "missing wallet_id in metadata"}))))?;
        let amount = intent["amount_received"].as_i64().unwrap_or(0);
        let pi_id = intent["id"].as_str().unwrap_or("");
        let idempotency_key = format!("stripe_pi_{}", pi_id);

        let wallet_id = Uuid::parse_str(wallet_id_str)
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))))?;

        let result = sqlx::query(
            "INSERT INTO wallet_transactions (id, wallet_id, type, amount_cents, stripe_payment_intent_id, idempotency_key)
             VALUES ($1, $2, 'topup', $3, $4, $5) ON CONFLICT (idempotency_key) DO NOTHING"
        )
            .bind(uuid::Uuid::new_v4())
            .bind(wallet_id)
            .bind(amount)
            .bind(pi_id)
            .bind(&idempotency_key)
            .execute(&state.db)
            .await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

        if result.rows_affected() > 0 {
            sqlx::query("UPDATE wallets SET balance_cents = balance_cents + $1, updated_at = now() WHERE id = $2")
                .bind(amount)
                .bind(wallet_id)
                .execute(&state.db)
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;
        }
    }

    Ok(Json(serde_json::json!({"status": "ok"})))
}
