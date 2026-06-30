use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
    http::StatusCode,
    Json,
};
use std::sync::Arc;

pub struct ProviderInfo {
    pub base_url: String,
    pub api_key: String,
}

pub fn get_provider_for_model(model: &str) -> ProviderInfo {
    if model.contains("deepseek") {
        ProviderInfo {
            base_url: "https://api.deepseek.com/v1".into(),
            api_key: std::env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
        }
    } else {
        ProviderInfo {
            base_url: "https://api.groq.com/openai/v1".into(),
            api_key: std::env::var("GROQ_API_KEY").unwrap_or_default(),
        }
    }
}
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub tenant_id: Option<String>,
    pub scope: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn create_jwt(secret: &str, sub: &str, tenant_id: Option<String>, scope: &str) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: sub.to_string(),
        tenant_id,
        scope: scope.to_string(),
        exp: now + 86400,
        iat: now,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| e.to_string())
}

pub fn verify_jwt(secret: &str, token: &str) -> Result<Claims, String> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ).map_err(|e| e.to_string())?;
    Ok(token_data.claims)
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let path = req.uri().path();

    let public_paths = [
        "/api/v1/health",
        "/api/v1/tenants/register",
        "/api/v1/stripe/webhook",
        "/api/v1/contact",
        "/api/v1/operator/login",
    ];

    if public_paths.iter().any(|p| path.starts_with(p)) {
        return Ok(next.run(req).await);
    }

    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "missing authorization header"})))
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid authorization format"})))
    })?;

    let claims = verify_jwt(&state.jwt_secret, token).map_err(|e| {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": e})))
    })?;

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
