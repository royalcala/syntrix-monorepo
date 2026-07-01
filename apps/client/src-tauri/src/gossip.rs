use crate::indexes::SqlEngine;

/// Process an incoming gossipsub event for a given org.
/// Called from the main event loop in identity.rs.
pub fn process_gossip_event(
    org_id: &str,
    val: &serde_json::Value,
    indexer: &SqlEngine,
) {
    // Heartbeat messages: { ts, status, node_id } — no "type" field
    if val.get("type").is_none() {
        if let Some(node_id) = val.get("node_id").and_then(|v| v.as_str()) {
            if let Some(_ts) = val.get("ts").and_then(|v| v.as_i64()) {
                let _ = indexer.upsert_heartbeat(org_id, node_id, val);
            }
        }
        return;
    }

    let event_type = match val.get("type").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return,
    };

    // Handle role.updated events
    if event_type == "role.updated" {
        if let Some(payload) = val.get("payload") {
            if let Some(role_name) = payload.get("name").and_then(|v| v.as_str()) {
                let role_cfg = serde_json::json!({
                    "name": role_name,
                    "can_open": payload.get("can_open"),
                    "can_write": payload.get("can_write"),
                });
                let _ = indexer.upsert_role_cfg(org_id, role_name, &role_cfg);
                let _ = indexer.append_event(org_id, val);
            }
        }
        return;
    }

    // Handle device.updated events
    if event_type == "device.updated" {
        if let Some(payload) = val.get("payload") {
            if let Some(node_id) = payload.get("node_id").and_then(|v| v.as_str()) {
                let _ = indexer.upsert_member(org_id, node_id, payload);
                let _ = indexer.append_event(org_id, val);
            }
        }
        return;
    }

    let entity = entity_from_event_type(event_type);
    let payload = match val.get("payload") {
        Some(p) => p.clone(),
        None => return,
    };

    let doc_id = payload.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    if let Some(hlc_val) = val.get("hlc") {
        let hlc = crate::indexes::HlcTimestamp {
            ts: hlc_val.get("ts").and_then(|v| v.as_u64()).unwrap_or(0),
            count: hlc_val.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            node: hlc_val.get("node").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };
        let _ = indexer.upsert_document_with_hlc(org_id, entity, &doc_id, &payload, &hlc);
    } else {
        let _ = indexer.upsert_document(org_id, entity, &doc_id, &payload);
    }

    let _ = indexer.append_event(org_id, val);
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
