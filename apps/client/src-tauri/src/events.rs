use std::sync::atomic::{AtomicU64, Ordering};

use crate::identity::AppState;
use syntrix_schema::{build_registry, can_access};

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

        let count = counter.fetch_add(1, Ordering::SeqCst) as u32;

        Self {
            ts: now,
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

fn schema_version_for(entity: &str) -> u32 {
    build_registry()
        .get(entity)
        .map(|s| s.version)
        .unwrap_or(1)
}

pub fn upcast_payload(entity: &str, payload: serde_json::Value, event_schema_version: u32) -> serde_json::Value {
    let current_version = schema_version_for(entity);
    if event_schema_version == current_version {
        return payload;
    }
    let upcasters = syntrix_schema::collect_upcasters();
    syntrix_schema::apply_upcasters(payload, event_schema_version, current_version, &upcasters)
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
    let _ = indexer.append_event(org_id, &value);

    let doc_id = payload_val.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload_val.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or(&key)
        .to_string();

    let upcasted = upcast_payload(entity, payload_val, schema_version);
    let _ = indexer.upsert_document(org_id, entity, &doc_id, &upcasted);

    if let Ok(event_bytes) = serde_json::to_vec(&value).map(bytes::Bytes::from) {
        let bus = state.gossip_bus.clone();
        let org = org_id.to_string();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut guard = bus.write().await;
                let _ = guard.broadcast(&org, event_bytes).await;
            });
        } else {
            let mut guard = bus.blocking_write();
            let _ = tauri::async_runtime::block_on(guard.broadcast(&org, event_bytes));
        }
    }

    Ok(key)
}
