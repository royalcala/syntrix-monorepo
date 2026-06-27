use std::sync::atomic::{AtomicU64, Ordering};

use crate::identity::AppState;
use syntrix_schema::{build_registry, can_access, upcast::{apply_upcasters, collect_upcasters}};

/// Hybrid Logical Clock — guarantees causal ordering without central authority.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Hlc {
    /// Microseconds since UNIX epoch.
    pub ts: u64,
    /// Monotonic counter (increments for events in the same microsecond).
    pub count: u32,
    /// Writer's node_id (hex encoded, for tiebreaking).
    pub node: String,
}

impl Hlc {
    /// Generate the next HLC for this device.
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

    /// Format as event key prefix for iroh-docs entries.
    pub fn to_key_prefix(&self) -> String {
        format!("evt:{:020}:{:08}:{}", self.ts, self.count, self.node)
    }
}

/// Determine the entity name from an event_type string (e.g. "customer.created" → "customers").
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

/// Route event_type to its iroh-docs namespace.
/// Returns (namespace_name, entity_name).
pub fn route_namespace(event_type: &str) -> Option<(&'static str, String)> {
    let entity = entity_from_event_type(event_type);
    let registry = build_registry();
    let ns = registry.namespace_of(entity)?;
    Some((ns.as_str(), entity.to_string()))
}

/// Get the current schema version for a given entity.
fn schema_version_for(entity: &str) -> u32 {
    build_registry()
        .get(entity)
        .map(|s| s.version)
        .unwrap_or(1)
}

/// Apply upcasters to bring a payload from an old schema version to the current version.
pub fn upcast_payload(entity: &str, payload: serde_json::Value, event_schema_version: u32) -> serde_json::Value {
    let current_version = schema_version_for(entity);
    if event_schema_version == current_version {
        return payload;
    }
    let upcasters = collect_upcasters();
    apply_upcasters(payload, event_schema_version, current_version, &upcasters)
}

/// Commit an event to the active org's data doc via iroh-docs.
pub fn commit_event(
    state: &AppState, event_type: &str, payload: &str,
) -> anyhow::Result<String> {
    let org_id = state.active_org().map_err(|e| anyhow::anyhow!("{}", e))?;
    let org = state.get_org_docs(org_id)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    let author = state.author();
    let node_id_hex = hex::encode(state.node_id());

    let entity = entity_from_event_type(event_type);

    let (ns_name, _entity_name) = route_namespace(event_type)
        .ok_or_else(|| anyhow::anyhow!("unknown event_type module: {}", event_type))?;

    let doc = match ns_name {
        "operational" => &org.operational_doc,
        "catalogs" => &org.catalogs_doc,
        "payroll" => &org.payroll_doc,
        _ => return Err(anyhow::anyhow!("unknown namespace: {}", ns_name)),
    };

    // Validate write permission (pure entity-level format)
    let role = &org.role;
    let role_key = format!("roles/{}", role);
    let mut can_write_flag = role == "admin";

    if !can_write_flag {
        if let Ok(stream_raw) = tauri::async_runtime::block_on(org.control_doc.get_many(iroh_docs::store::Query::key_exact(role_key.clone()))) {
            let mut stream = Box::pin(stream_raw);
            use futures_util::stream::StreamExt;
            if let Some(Ok(entry)) = tauri::async_runtime::block_on(stream.next()) {
                let hash = entry.content_hash();
                if let Ok(bytes) = tauri::async_runtime::block_on(state.store().blobs().get_bytes(hash)) {
                    let bytes_ref: &[u8] = bytes.as_ref();
                    if let Ok(grants) = serde_json::from_slice::<serde_json::Value>(bytes_ref) {
                        if let Some(write_perms) = grants.get("can_write").and_then(|v| v.as_array()) {
                            let perms: Vec<String> = write_perms.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                            can_write_flag = can_access(&perms, entity);
                        }
                    }
                }
            }
        }
    }

    if !can_write_flag {
        return Err(anyhow::anyhow!("Write denied: role {} cannot write to {}", role, entity));
    }

    let hlc = Hlc::next(&node_id_hex, state.counter());
    let key = hlc.to_key_prefix();

    let payload_val = serde_json::from_str::<serde_json::Value>(payload)?;
    let schema_version = schema_version_for(entity);

    let value = serde_json::json!({
        "type": event_type,
        "hlc": hlc,
        "schema_version": schema_version,
        "payload": &payload_val,
    });

    tauri::async_runtime::block_on(
        doc.set_bytes(author, key.clone().into_bytes(), serde_json::to_vec(&value)?)
    )?;

    // Index the new document (apply upcasters first for safety)
    let doc_id = payload_val.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload_val.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or(&key)
        .to_string();

    let upcasted = upcast_payload(entity, payload_val, schema_version);
    let _ = state.indexer.upsert_document(&org_id, entity, &doc_id, &upcasted);

    Ok(key)
}
