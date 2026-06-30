use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub org_id: String,
    pub name: Option<String>,
    pub public_key_hex: String,
    pub api_key_hash: String,
    pub status: Option<String>,
    pub wallet_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Wallet {
    pub id: Uuid,
    pub balance_cents: i64,
    pub reserved_cents: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WalletTransaction {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub r#type: String,
    pub amount_cents: i64,
    pub service: Option<String>,
    pub stripe_payment_intent_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServiceRateCard {
    pub id: Uuid,
    pub service: String,
    pub unit_label: String,
    pub rate_cents_per_unit: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServiceUsageLog {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub service: String,
    pub quantity: i64,
    pub cost_cents: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GatewayApiKey {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub key_hash: String,
    pub name: Option<String>,
    pub rate_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CfdiStamp {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub pac_reference: String,
    pub cfdi_uuid: Option<String>,
    pub xml_in: Option<String>,
    pub xml_out: Option<String>,
    pub status: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Operator {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub totp_secret: Option<String>,
    pub totp_verified: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ContactInquiry {
    pub id: Uuid,
    pub name: Option<String>,
    pub email: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
}
