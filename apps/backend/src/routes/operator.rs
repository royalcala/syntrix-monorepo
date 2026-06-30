use axum::{extract::{State, Path}, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;
use crate::auth;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub jwt: String,
    pub operator_id: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let operator = sqlx::query_as::<_, crate::db::models::Operator>(
        "SELECT * FROM operators WHERE email = $1"
    )
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?
        .ok_or_else(|| (axum::http::StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))))?;

    let valid = argon2::verify_encoded(&operator.password_hash, req.password.as_bytes())
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    if !valid {
        return Err((axum::http::StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))));
    }

    let jwt = auth::create_jwt(&state.jwt_secret, &operator.id.to_string(), None, "operator")
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;

    Ok(Json(LoginResponse {
        jwt,
        operator_id: operator.id.to_string(),
    }))
}

#[derive(Serialize)]
pub struct TenantSummary {
    pub id: String,
    pub org_id: String,
    pub name: Option<String>,
    pub status: Option<String>,
    pub wallet_balance_cents: i64,
    pub created_at: String,
}

pub async fn list_tenants(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TenantSummary>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let tenants = sqlx::query_as::<_, crate::db::models::Tenant>(
        "SELECT t.* FROM tenants t ORDER BY t.created_at DESC"
    )
        .fetch_all(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let mut summaries = Vec::new();
    for t in tenants {
        let balance = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(w.balance_cents, 0) FROM wallets w WHERE w.id = $1"
        )
            .bind(t.wallet_id)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

        summaries.push(TenantSummary {
            id: t.id.to_string(),
            org_id: t.org_id,
            name: t.name,
            status: t.status,
            wallet_balance_cents: balance,
            created_at: t.created_at.to_rfc3339(),
        });
    }

    Ok(Json(summaries))
}

#[derive(Serialize)]
pub struct TenantDetail {
    pub id: String,
    pub org_id: String,
    pub name: Option<String>,
    pub status: Option<String>,
    pub wallet_balance_cents: i64,
    pub reserved_cents: i64,
    pub transactions: Vec<serde_json::Value>,
    pub usage: Vec<serde_json::Value>,
    pub created_at: String,
}

pub async fn tenant_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<TenantDetail>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let tenant = sqlx::query_as::<_, crate::db::models::Tenant>(
        "SELECT * FROM tenants WHERE id = $1"
    )
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "tenant not found"}))))?;

    let wallet = sqlx::query_as::<_, crate::db::models::Wallet>(
        "SELECT * FROM wallets WHERE id = $1"
    )
        .bind(tenant.wallet_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?
        .unwrap_or(crate::db::models::Wallet {
            id: uuid::Uuid::default(),
            balance_cents: 0,
            reserved_cents: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        });

    let transactions = sqlx::query_as::<_, crate::db::models::WalletTransaction>(
        "SELECT * FROM wallet_transactions WHERE wallet_id = $1 ORDER BY created_at DESC LIMIT 50"
    )
        .bind(tenant.wallet_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let usage = sqlx::query_as::<_, crate::db::models::ServiceUsageLog>(
        "SELECT * FROM service_usage_log WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT 50"
    )
        .bind(tenant.id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let tx_values: Vec<serde_json::Value> = transactions.into_iter().map(|t| {
        serde_json::json!({
            "id": t.id.to_string(),
            "type": t.r#type,
            "amount_cents": t.amount_cents,
            "service": t.service,
            "created_at": t.created_at.to_rfc3339(),
        })
    }).collect();

    let usage_values: Vec<serde_json::Value> = usage.into_iter().map(|u| {
        serde_json::json!({
            "service": u.service,
            "quantity": u.quantity,
            "cost_cents": u.cost_cents,
            "created_at": u.created_at.to_rfc3339(),
        })
    }).collect();

    Ok(Json(TenantDetail {
        id: tenant.id.to_string(),
        org_id: tenant.org_id,
        name: tenant.name,
        status: tenant.status,
        wallet_balance_cents: wallet.balance_cents,
        reserved_cents: wallet.reserved_cents,
        transactions: tx_values,
        usage: usage_values,
        created_at: tenant.created_at.to_rfc3339(),
    }))
}

#[derive(Serialize)]
pub struct RateCardResponse {
    pub id: String,
    pub service: String,
    pub unit_label: String,
    pub rate_cents_per_unit: i64,
}

pub async fn list_rate_cards(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RateCardResponse>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let cards = sqlx::query_as::<_, crate::db::models::ServiceRateCard>(
        "SELECT * FROM service_rate_cards ORDER BY service"
    )
        .fetch_all(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(cards.into_iter().map(|c| RateCardResponse {
        id: c.id.to_string(),
        service: c.service,
        unit_label: c.unit_label,
        rate_cents_per_unit: c.rate_cents_per_unit,
    }).collect()))
}

#[derive(Deserialize)]
pub struct UpdateRateCardRequest {
    pub rate_cents_per_unit: i64,
}

pub async fn update_rate_card(
    State(state): State<Arc<AppState>>,
    Path(service): Path<String>,
    Json(req): Json<UpdateRateCardRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    sqlx::query("UPDATE service_rate_cards SET rate_cents_per_unit = $1 WHERE service = $2")
        .bind(req.rate_cents_per_unit)
        .bind(&service)
        .execute(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(serde_json::json!({"status": "updated"})))
}
