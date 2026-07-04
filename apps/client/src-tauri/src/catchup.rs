use syntrix_network::P2PNode;
use crate::indexes::SqlEngine;

/// Requests a catchup snapshot from `peer_id` and applies every returned event locally
/// (Fase 3, tarea 16). The admin's response mixes two kinds of items:
///   - Device/role roster events (`{"type":"role.updated"|"device.updated","payload":...}`),
///     needed so a peer can validate `apply_cdc_events`'s author permission check for *any*
///     other peer's writes, not just ones it happened to be subscribed for when broadcast.
///   - A relational snapshot (`{"kind":"cdc_batch","events":[CdcEvent...]}`) of the org's
///     current entity/child rows, applied via the exact same `apply_cdc_events` used for live
///     CDC sync — so catch-up and live sync share identical LWW/permission-checking behavior.
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

/// Applies a single catchup item: a device/role roster event, or a `cdc_batch` relational
/// snapshot. Shared by the join-time and restart-time catchup call sites
/// (lib.rs::join_org_impl, identity.rs's on-restart catchup).
pub fn apply_catchup_event(org_id: &str, event: &serde_json::Value, indexer: &SqlEngine) {
    if event.get("kind").and_then(|k| k.as_str()) == Some("cdc_batch") {
        let events: Vec<syntrix_network::cdc::CdcEvent> = match event.get("events").cloned() {
            Some(v) => match serde_json::from_value(v) {
                Ok(events) => events,
                Err(e) => {
                    tracing::warn!(target: "syntrix", error = %e, "catchup: failed to decode cdc_batch snapshot");
                    return;
                }
            },
            None => return,
        };
        let perm = |node_id_hex: &str, entity: &str| indexer.can_node_write(org_id, node_id_hex, entity);
        if let Err(e) = indexer.apply_cdc_events(&events, &perm) {
            tracing::warn!(target: "syntrix", org = %org_id, error = %e, "catchup: failed to apply cdc_batch snapshot");
        }
        return;
    }

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
    fn apply_catchup_event_handles_role_updated_and_device_updated() {
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
    }

    #[test]
    fn apply_catchup_event_applies_cdc_batch_snapshot() {
        let (_dir, engine) = test_engine();

        // In the real catchup flow, roster items (device.updated/role.updated) are applied
        // before the cdc_batch snapshot (see admin's `CatchupRequestReceived` handler), so
        // `can_node_write` can already validate the snapshot rows' authors. Seed the same
        // roster here to mirror that ordering rather than testing an unrealistic state.
        apply_catchup_event("org1", &serde_json::json!({
            "type": "role.updated",
            "payload": { "name": "sales", "can_open": ["*"], "can_write": ["customers"] },
        }), &engine);
        apply_catchup_event("org1", &serde_json::json!({
            "type": "device.updated",
            "payload": { "node_id": "nodeA", "active": true, "role": "sales" },
        }), &engine);

        let mut cols = std::collections::BTreeMap::new();
        cols.insert("org_id".to_string(), serde_json::json!("org1"));
        cols.insert("doc_id".to_string(), serde_json::json!("c1"));
        cols.insert("name".to_string(), serde_json::json!("Alice"));
        cols.insert("tax_id".to_string(), serde_json::Value::Null);
        cols.insert("address".to_string(), serde_json::Value::Null);
        cols.insert("phone".to_string(), serde_json::Value::Null);
        cols.insert("email".to_string(), serde_json::json!("alice@test.com"));
        cols.insert("change_time".to_string(), serde_json::json!(1000));
        cols.insert("node_id".to_string(), serde_json::json!("nodeA"));
        let snapshot_event = syntrix_network::cdc::CdcEvent {
            table: "customers".to_string(),
            change_type: 1,
            change_time: 1000,
            columns: cols,
        };

        apply_catchup_event("org1", &serde_json::json!({
            "kind": "cdc_batch",
            "events": [snapshot_event],
        }), &engine);

        let doc = engine.get_document("org1", "customers", "c1").unwrap().unwrap();
        assert_eq!(doc["name"], "Alice");
        assert_eq!(doc["email"], "alice@test.com");
    }
}
