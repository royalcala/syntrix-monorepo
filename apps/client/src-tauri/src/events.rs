use std::sync::atomic::{AtomicU64, Ordering};

use crate::identity::AppState;
use syntrix_core::{can_access, schema_version_for};

// Tracks the highest wall-clock timestamp ever emitted, so HLC never goes
// backward even if the system clock jumps backwards (NTP correction, suspend).
static LAST_HLC_TS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Hlc {
    pub ts: u64,
    pub count: u32,
    pub node: String,
}

impl Hlc {
    pub fn next(node_id_hex: &str, counter: &AtomicU64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let ts = LAST_HLC_TS.fetch_max(now, Ordering::SeqCst).max(now);
        let count = counter.fetch_add(1, Ordering::SeqCst) as u32;

        Self {
            ts,
            count,
            node: node_id_hex[..16].to_string(),
        }
    }
}

pub fn entity_from_event_type(event_type: &str) -> &str {
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

pub fn upcast_payload(entity: &str, payload: serde_json::Value, event_schema_version: u32) -> serde_json::Value {
    let current_version = syntrix_core::schema_version_for(entity);
    if event_schema_version == current_version {
        return payload;
    }
    syntrix_core::upcast_payload(payload, event_schema_version, current_version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn test_entity_from_event_type() {
        assert_eq!(entity_from_event_type("customer.created"), "customers");
        assert_eq!(entity_from_event_type("invoice.paid"), "invoices");
        assert_eq!(entity_from_event_type("order.shipped"), "orders");
        assert_eq!(entity_from_event_type("product.updated"), "products");
        assert_eq!(entity_from_event_type("payroll.processed"), "payroll");
        assert_eq!(entity_from_event_type("unknown.action"), "unknown");
    }

    #[test]
    fn test_hlc_next_monotonic() {
        let node_id = "abcdef1234567890abcdef1234567890";
        let counter = AtomicU64::new(0);
        let a = Hlc::next(node_id, &counter);
        let b = Hlc::next(node_id, &counter);
        assert!(b.ts >= a.ts, "timestamps should be monotonic");
        assert_eq!(b.count, 1);
        assert_eq!(a.node, "abcdef1234567890".to_string());
    }

    #[test]
    fn test_hlc_next_different_nodes() {
        let counter = AtomicU64::new(0);
        let a = Hlc::next("aaaa1111aaaa1111aaaa1111aaaa1111", &counter);
        let b = Hlc::next("bbbb2222bbbb2222bbbb2222bbbb2222", &counter);
        assert_eq!(a.count, 0);
        assert_eq!(b.count, 1);
        assert_eq!(a.node, "aaaa1111aaaa1111");
        assert_eq!(b.node, "bbbb2222bbbb2222");
    }

    #[test]
    fn test_upcast_payload_same_version() {
        let payload = serde_json::json!({"id": "c1", "name": "test"});
        let result = upcast_payload("customers", payload.clone(), 1);
        assert_eq!(result, payload);
    }
}

pub fn commit_event(
    state: &AppState, event_type: &str, payload: &str,
) -> anyhow::Result<String> {
    let org_id = state.active_org().map_err(|e| anyhow::anyhow!("{}", e))?;
    let org = state.get_org(org_id)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    let node_id_hex = hex::encode(state.node_id());

    let entity = entity_from_event_type(event_type);
    let role = &org.role;

    let can_write_flag = role == "admin" || {
        let indexer = state.indexer();
        let roles = indexer.get_roles(org_id).unwrap_or_default();
        roles.iter().any(|r| {
            r.get("name").and_then(|n| n.as_str()) == Some(role.as_str())
                && can_access(
                    &r.get("can_write")
                        .and_then(|a| a.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                        .unwrap_or_default(),
                    entity,
                )
        })
    };

    if !can_write_flag {
        return Err(anyhow::anyhow!("Write denied: role {} cannot write to {}", role, entity));
    }

    let hlc = Hlc::next(&node_id_hex, state.counter());
    let key = format!("evt:{}:{:020}:{:08}:{}", org_id, hlc.ts, hlc.count, hlc.node);

    let payload_val = serde_json::from_str::<serde_json::Value>(payload)?;
    let schema_version = schema_version_for(entity);

    let value = serde_json::json!({
        "type": event_type,
        "hlc": hlc,
        "schema_version": schema_version,
        "payload": &payload_val,
    });

    let indexer = state.indexer();

    let doc_id = payload_val.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload_val.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or(&key)
        .to_string();

    let upcasted = upcast_payload(entity, payload_val, schema_version);
    // Typed SQL write: business fields -> real columns (Fase 2, tarea 11). `change_time`
    // comes from the HLC's microsecond timestamp (converted to millis, matching the column's
    // unit) so the row is immediately consistent with what CDC will later replay to peers.
    let change_time_millis = (hlc.ts / 1000) as i64;
    let _ = indexer.upsert_document_full(org_id, entity, &doc_id, &upcasted, change_time_millis, &node_id_hex);

    state.live_manager().notify_table_changed(&state.conn(), &[entity.to_string()]);

    // NOTE (Fase 3, Decisión 4): entity data no longer propagates via an immediate gossip
    // publish of this JSON event. The typed row just written above (via
    // `upsert_document_full`) is picked up by `turso_cdc`, and the periodic CDC publish loop
    // (cdc_sync.rs::run_cdc_publish_loop) reads and gossips it to peers as a `CdcEvent`. This
    // event_log entry remains client-local, informational only (see audit.rs).

    Ok(key)
}
