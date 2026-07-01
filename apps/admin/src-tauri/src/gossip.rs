use std::num::NonZero;
use std::sync::Arc;

pub fn write_event_to_limbo(db: &Arc<turso_core::Connection>, org_id: &str, val: &serde_json::Value) {
    let event_type = val["type"].as_str().unwrap_or("unknown");
    let hlc_val = &val["hlc"];
    let hlc_ts = hlc_val["ts"].as_u64().unwrap_or(0);
    let hlc_count = hlc_val["count"].as_u64().unwrap_or(0);
    let hlc_node = hlc_val["node"].as_str().unwrap_or("");
    let schema_version = val["schema_version"].as_u64().unwrap_or(1);
    let payload = val.get("payload").cloned().unwrap_or_default();
    let payload_str = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[admin-gossip] failed to serialize payload: {e}");
            return;
        }
    };
    let entity = entity_from_event_type(event_type);

    let key = format!(
        "evt:{}:{:020}:{:08}:{}",
        org_id, hlc_ts, hlc_count, hlc_node
    );

    let mut stmt = match db.prepare(
        "INSERT INTO event_log (org_id, key, event_type, hlc_ts, hlc_count, hlc_node, schema_version, entity, payload) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
            return;
        }
    };

    if let Err(e) = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string())) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }
    if let Err(e) = stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(key.clone())) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }
    if let Err(e) = stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(event_type.to_string())) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }
    if let Err(e) = stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_i64(hlc_ts as i64)) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }
    if let Err(e) = stmt.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_i64(hlc_count as i64)) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }
    if let Err(e) = stmt.bind_at(NonZero::new(6).unwrap(), turso_core::Value::from_text(hlc_node.to_string())) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }
    if let Err(e) = stmt.bind_at(NonZero::new(7).unwrap(), turso_core::Value::from_i64(schema_version as i64)) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }
    if let Err(e) = stmt.bind_at(NonZero::new(8).unwrap(), turso_core::Value::from_text(entity.to_string())) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }
    if let Err(e) = stmt.bind_at(NonZero::new(9).unwrap(), turso_core::Value::from_text(payload_str.clone())) {
        eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
        return;
    }

    loop {
        match stmt.step() {
            Ok(turso_core::StepResult::Row) => continue,
            Ok(turso_core::StepResult::Done) => break,
            Ok(turso_core::StepResult::IO | turso_core::StepResult::Yield) => {
                if let Err(e) = stmt._io().step() {
                    eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
                    return;
                }
            }
            Ok(turso_core::StepResult::Interrupt | turso_core::StepResult::Busy) => {
                continue;
            }
            Err(e) => {
                eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
                return;
            }
        }
    }

    tracing::info!(
        target: "syntrix",
        org = %org_id,
        event_type = %event_type,
        "admin-audit: stored gossip event in event_log"
    );
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
