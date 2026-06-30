mod db;
mod auth;
mod routes;

use std::sync::Arc;
use axum::{Router, middleware};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

pub struct AppState {
    pub db: sqlx::PgPool,
    pub jwt_secret: String,
    pub stripe_secret: String,
    pub stripe_webhook_secret: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("syntrix_backend=info".parse()?))
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://syntrix:password@localhost:5432/syntrix".into());
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production".into());
    let stripe_secret = std::env::var("STRIPE_SECRET_KEY")
        .unwrap_or_default();
    let stripe_webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET")
        .unwrap_or_default();

    let db = sqlx::PgPool::connect(&database_url).await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    let state = Arc::new(AppState {
        db,
        jwt_secret,
        stripe_secret,
        stripe_webhook_secret,
    });

    let app = Router::new()
        .route("/api/v1/health", axum::routing::get(health_check))
        .route("/api/v1/tenants/register", axum::routing::post(routes::tenants::register))
        .route("/api/v1/operator/login", axum::routing::post(routes::operator::login))
        .route("/api/v1/contact", axum::routing::post(routes::contact::submit))
        .route("/api/v1/wallet", axum::routing::get(routes::wallet::get_balance))
        .route("/api/v1/wallet/topup", axum::routing::post(routes::wallet::create_topup))
        .route("/api/v1/wallet/transactions", axum::routing::get(routes::wallet::get_transactions))
        .route("/api/v1/stripe/webhook", axum::routing::post(routes::wallet::stripe_webhook))
        .route("/api/v1/gateway/chat/completions", axum::routing::post(routes::gateway::chat_completions))
        .route("/api/v1/cfdi/stamp", axum::routing::post(routes::cfdi::stamp))
        .route("/api/v1/cfdi/{uuid}/status", axum::routing::get(routes::cfdi::status))
        .route("/api/v1/email/send", axum::routing::post(routes::email::send))
        .route("/api/v1/sms/send", axum::routing::post(routes::sms::send))
        .route("/api/v1/key-recovery/store", axum::routing::post(routes::key_recovery::store))
        .route("/api/v1/key-recovery/retrieve", axum::routing::get(routes::key_recovery::retrieve))
        .route("/api/v1/operator/tenants", axum::routing::get(routes::operator::list_tenants))
        .route("/api/v1/operator/tenants/{id}", axum::routing::get(routes::operator::tenant_detail))
        .route("/api/v1/operator/rate-cards", axum::routing::get(routes::operator::list_rate_cards).put(routes::operator::update_rate_card))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    tracing::info!("listening on 0.0.0.0:3001");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({ "status": "ok" }))
}
