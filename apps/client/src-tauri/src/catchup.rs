use std::sync::Arc;

use syntrix_network::P2PNode;
use crate::indexes::SqlEngine;

/// Requests a catchup snapshot from `peer_id` and applies every returned event locally,
/// including device/role roster events (Fase 3, tarea 16: catchup must actually restore
/// state for peers outside `turso_cdc` retention, and — per the new permission validation in
/// `apply_cdc_events`/`identity.rs::apply_cdc_batch` — every peer needs to learn the
/// roles/devices roster for the org, not just entity documents).
pub async fn request_catchup(
    p2p: &P2PNode,
    peer_id: libp2p::PeerId,
    org_id: &str,
    since_hlc: u64,
    indexer: &SqlEngine,
) -> anyhow::Result<usize> {
    let events = p2p.request_catchup(peer_id, org_id.to_string(), since_hlc).await?;
    let count = events.len();
    for event in &events {
        apply_catchup_event(org_id, event, indexer);
    }
    Ok(count)
}

/// Applies a single catchup event, dispatching device/role roster snapshots and entity
/// documents alike. Shared by the join-time and restart-time catchup call sites
/// (lib.rs::join_org_impl, identity.rs's on-restart catchup) so both learn the org's
/// roles/devices roster, not just entity data.
pub fn apply_catchup_event(org_id: &str, event: &serde_json::Value, indexer: &SqlEngine) {
    let event_type = event["type"].as_str().unwrap_or("");

    if event_type == "role.updated" {
        if let Some(payload) = event.get("payload") {
            if let Some(role_name) = payload.get("name").and_then(|v| v.as_str()) {
                let role_cfg = serde_json::json!({
                    "name": role_name,
                    "can_open": payload.get("can_open"),
                    "can_write": payload.get("can_write"),
                });
                let _ = indexer.upsert_role_cfg(org_id, role_name, &role_cfg);
            }
        }
        return;
    }

    if event_type == "device.updated" {
        if let Some(payload) = event.get("payload") {
            if let Some(node_id) = payload.get("node_id").and_then(|v| v.as_str()) {
                let _ = indexer.upsert_member(org_id, node_id, payload);
            }
        }
        return;
    }

    let entity = entity_from_event_type(event_type);
    let payload = match event.get("payload") {
        Some(p) => p.clone(),
        None => return,
    };
    let doc_id = payload.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    if let Some(hlc_val) = event.get("hlc") {
        let hlc = crate::indexes::HlcTimestamp {
            ts: hlc_val.get("ts").and_then(|v| v.as_u64()).unwrap_or(0),
            count: hlc_val.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            node: hlc_val.get("node").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };
        let _ = indexer.upsert_document_with_hlc(org_id, entity, &doc_id, &payload, &hlc);
    } else {
        let _ = indexer.upsert_document(org_id, entity, &doc_id, &payload);
    }
}

fn entity_from_event_type(event_type: &str) -> &str {
    match event_type.split('.').next().unwrap_or(event_type) {
        "invoice" | "invoices" => "invoices",
        "order" | "orders" => "orders",
        "product" | "products" => "products",
        "customer" | "customers" => "customers",
        "supplier" | "suppliers" => "suppliers",
        "payroll" => "payroll",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> (impl Drop, SqlEngine) {
        let (dir, conn) = syntrix_testkit::temp_limbo_db();
        crate::storage::run_migrations(&conn).expect("run_migrations");
        (dir, SqlEngine::with_connection(conn))
    }

    #[test]
    fn apply_catchup_event_handles_device_updated_role_updated_and_entity() {
        let (_dir, engine) = test_engine();

        apply_catchup_event("org1", &serde_json::json!({
            "type": "role.updated",
            "payload": { "name": "sales", "can_open": ["*"], "can_write": ["customers"] },
        }), &engine);
        let roles = engine.get_roles("org1").unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0]["name"], "sales");

        apply_catchup_event("org1", &serde_json::json!({
            "type": "device.updated",
            "payload": { "node_id": "abcd", "active": true, "role": "sales" },
        }), &engine);
        let members = engine.get_members("org1").unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["node_id"], "abcd");

        apply_catchup_event("org1", &serde_json::json!({
            "type": "customer.created",
            "hlc": { "ts": 1000, "count": 0, "node": "abcd" },
            "payload": { "id": "c1", "name": "Alice" },
        }), &engine);
        let doc = engine.get_document("org1", "customers", "c1").unwrap().unwrap();
        assert_eq!(doc["name"], "Alice");
    }
}
