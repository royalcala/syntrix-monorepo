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

    // Route event to the correct namespace
    let module_name = match event_type.split('.').next().unwrap_or(event_type) {
        "invoice" | "invoices" | "upsert_invoices" => "invoices",
        "order" | "orders" | "upsert_orders" => "orders",
        "product" | "products" | "upsert_products" => "products",
        "customer" | "customers" | "upsert_customers" => "customers",
        "supplier" | "suppliers" | "upsert_suppliers" => "suppliers",
        "payroll" | "upsert_payroll" => "payroll",
        _ => event_type,
    };

    let (doc, ns_name) = match module_name {
        "invoices" | "orders" | "sales_note" => (&org.operational_doc, "operational"),
        "products" | "customers" | "suppliers" | "chart_of_accounts" => (&org.catalogs_doc, "catalogs"),
        "payroll" => (&org.payroll_doc, "payroll"),
        _ => return Err(anyhow::anyhow!("unknown event_type module: {}", module_name)),
    };

    // Validate write permission (allow if they have module permission OR namespace permission)
    let role = &org.role;
    let role_key = format!("roles/{}", role);
    let mut can_write = role == "admin"; // Admin bypass

    if !can_write {
        if let Ok(stream_raw) = tauri::async_runtime::block_on(org.control_doc.get_many(iroh_docs::store::Query::key_exact(role_key.clone()))) {
            let mut stream = Box::pin(stream_raw);
            use futures_util::stream::StreamExt;
            if let Some(Ok(entry)) = tauri::async_runtime::block_on(stream.next()) {
                let hash = entry.content_hash();
                if let Ok(bytes) = tauri::async_runtime::block_on(state.store().blobs().get_bytes(hash)) {
                    let bytes_ref: &[u8] = bytes.as_ref();
                    if let Ok(grants) = serde_json::from_slice::<serde_json::Value>(bytes_ref) {
                        if let Some(write_perms) = grants.get("can_write").and_then(|v| v.as_array()) {
                            let perms: Vec<&str> = write_perms.iter().filter_map(|v| v.as_str()).collect();
                            can_write = perms.contains(&module_name) || perms.contains(&ns_name) || perms.contains(&"*");
                        }
                    }
                }
            }
        }
    }

    if !can_write {
        return Err(anyhow::anyhow!("Write denied: role cannot write to module {} or namespace {}", module_name, ns_name));
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
