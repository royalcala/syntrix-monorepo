use std::sync::atomic::{AtomicU64, Ordering};

use crate::identity::AppState;

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

/// Commit an event to the active org's data doc via iroh-docs.
pub fn commit_event(
    state: &AppState, event_type: &str, payload: &str,
) -> anyhow::Result<String> {
    let org_id = state.active_org().map_err(|e| anyhow::anyhow!("{}", e))?;
    let org = state.get_org_docs(org_id)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    let author = state.author();
    let node_id_hex = hex::encode(state.node_id());
    let node_id = state.node_id();

    // Route event to the correct namespace
    let (doc, ns_name) = match event_type.split('.').next().unwrap_or(event_type) {
        "invoice" | "order" | "sales_note" => (&org.operational_doc, "operational"),
        "product" | "customer" | "chart_of_accounts" => (&org.catalogs_doc, "catalogs"),
        "payroll" => (&org.payroll_doc, "payroll"),
        _ => return Err(anyhow::anyhow!("unknown event_type: {}", event_type)),
    };

    // Validate write permission
    let can_write = state.registry().read()
        .map(|r| r.can_write(&org_id.into(), &node_id, ns_name))
        .unwrap_or(false);
    if !can_write {
        return Err(anyhow::anyhow!("Write denied: role cannot write to {} namespace", ns_name));
    }

    let hlc = Hlc::next(&node_id_hex, state.counter());
    let key = hlc.to_key_prefix();

    let payload_val = serde_json::from_str::<serde_json::Value>(payload)?;

    let value = serde_json::json!({
        "type": event_type,
        "hlc": hlc,
        "payload": &payload_val,
    });

    tauri::async_runtime::block_on(
        doc.set_bytes(author, key.clone().into_bytes(), serde_json::to_vec(&value)?)
    )?;

    // Index the new document
    let entity = event_type.split('.').next().unwrap_or(event_type);
    let doc_id = payload_val.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload_val.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or(&key);
        
    let _ = state.indexer.upsert_document(&org_id, entity, doc_id, &payload_val);

    Ok(key)
}
