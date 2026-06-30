use axum::{extract::{State, Path}, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct StampRequest {
    pub xml: String,
}

#[derive(Serialize)]
pub struct StampResponse {
    pub id: String,
    pub cfdi_uuid: String,
    pub xml_out: String,
}

pub async fn stamp(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StampRequest>,
) -> Result<Json<StampResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    // Mock PAC implementation — returns dummy timbre
    let stamp_id = Uuid::new_v4();
    let cfdi_uuid = Uuid::new_v4().to_string();

    let _ = sqlx::query(
        "INSERT INTO cfdi_stamps (id, tenant_id, pac_reference, cfdi_uuid, xml_in, xml_out, status) VALUES ($1, $2, $3, $4, $5, $6, 'completed')"
    )
        .bind(stamp_id)
        .bind(Uuid::nil()) // TODO: extract tenant from claims
        .bind(format!("mock_pac_ref_{}", stamp_id))
        .bind(&cfdi_uuid)
        .bind(&req.xml)
        .bind(&req.xml) // mock: same XML
        .execute(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(StampResponse {
        id: stamp_id.to_string(),
        cfdi_uuid,
        xml_out: req.xml,
    }))
}

#[derive(Serialize)]
pub struct CfdiStatus {
    pub id: String,
    pub cfdi_uuid: Option<String>,
    pub status: String,
    pub created_at: String,
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
) -> Result<Json<CfdiStatus>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let stamp = sqlx::query_as::<_, crate::db::models::CfdiStamp>(
        "SELECT * FROM cfdi_stamps WHERE cfdi_uuid = $1"
    )
        .bind(&uuid)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "stamp not found"}))))?;

    Ok(Json(CfdiStatus {
        id: stamp.id.to_string(),
        cfdi_uuid: stamp.cfdi_uuid,
        status: stamp.status.unwrap_or("unknown".into()),
        created_at: stamp.created_at.to_rfc3339(),
    }))
}
