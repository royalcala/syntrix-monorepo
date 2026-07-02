use std::num::NonZero;
use std::sync::{Arc, RwLock};

use syntrix_core::registry::NamespaceRegistry;
use syntrix_network::cdc::CdcEvent;

/// Writes an incoming event to the admin `event_log` **data-audit** table (Decision 5:
/// `event_log` is no longer a sync transport, only a durable history of record-level
/// changes). Until Fase 3 wires the real CDC-native gossip transport, this still consumes
/// the legacy JSON business-event stream published by the client's `commit_event`
/// (events.rs) and reconstructs an approximate data-audit row from it: `change_type` is
/// best-effort ("write", since the legacy event stream does not carry insert/update/delete
/// distinctly) and `row_image` is the event payload as received. Once the CDC loop lands,
/// this function is replaced by ingesting real `CdcEvent`s (see
/// crates/syntrix-network/src/cdc.rs) with accurate change_type/row image.
pub fn write_event_to_limbo(db: &Arc<turso_core::Connection>, org_id: &str, val: &serde_json::Value) {
    let event_type = val["type"].as_str().unwrap_or("unknown");
    let hlc_val = &val["hlc"];
    let hlc_ts = hlc_val["ts"].as_u64().unwrap_or(0);
    let hlc_node = hlc_val["node"].as_str().unwrap_or("");
    let payload = val.get("payload").cloned().unwrap_or_default();
    let entity = entity_from_event_type(event_type);
    let doc_id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    // `row_image` retains the full original event (type/hlc/payload) purely as an audit
    // record — catch-up no longer replays `event_log` (see identity.rs's
    // `CatchupRequestReceived` handler, which now sends a relational snapshot via
    // `syntrix_network::cdc::snapshot_org_rows` instead).
    let row_image = val.clone();
    let row_image_str = match serde_json::to_string(&row_image) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[admin-gossip] failed to serialize row image: {e}");
            return;
        }
    };

    let change_time_millis = (hlc_ts / 1000) as i64;

    let mut stmt = match db.prepare(
        "INSERT INTO event_log (org_id, entity, change_type, doc_id, row_image, change_time, node_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
            return;
        }
    };

    let binds: [(usize, turso_core::Value); 7] = [
        (1, turso_core::Value::from_text(org_id.to_string())),
        (2, turso_core::Value::from_text(entity.to_string())),
        (3, turso_core::Value::from_text("write".to_string())),
        (4, turso_core::Value::from_text(doc_id)),
        (5, turso_core::Value::from_text(row_image_str)),
        (6, turso_core::Value::from_i64(change_time_millis)),
        (7, turso_core::Value::from_text(hlc_node.to_string())),
    ];
    for (idx, value) in binds {
        if let Err(e) = stmt.bind_at(NonZero::new(idx).unwrap(), value) {
            eprintln!("[admin-gossip] failed to write event {} to event_log: {e}", event_type);
            return;
        }
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

/// Applies an incoming CDC batch to the admin's own relational replica (Fase 4, tarea 19),
/// and records each accepted change as a data-audit row in `event_log` (tarea 20). The admin's
/// `NamespaceRegistry` is authoritative and complete (unlike a client's, which only knows its
/// own device) since admin issues every `device.updated`/`role.updated`, so it can validate
/// the author's write permission directly.
pub fn apply_cdc_batch(
    db: &Arc<turso_core::Connection>,
    org_id: &str,
    val: &serde_json::Value,
    registry: &Arc<RwLock<NamespaceRegistry>>,
) {
    let events: Vec<CdcEvent> = match val.get("events").cloned() {
        Some(v) => match serde_json::from_value(v) {
            Ok(events) => events,
            Err(e) => {
                tracing::warn!(target: "syntrix", error = %e, "admin apply_cdc_batch: failed to decode events");
                return;
            }
        },
        None => return,
    };

    let org_id_owned = org_id.to_string();
    let registry_clone = registry.clone();
    let perm = move |node_id_hex: &str, entity: &str| {
        let Ok(node_bytes) = hex::decode(node_id_hex) else { return false };
        if node_bytes.len() != 32 {
            return false;
        }
        let mut node_id = [0u8; 32];
        node_id.copy_from_slice(&node_bytes);
        match registry_clone.read() {
            Ok(reg) => reg.can_write(&org_id_owned, &node_id, entity),
            Err(_) => false,
        }
    };

    // Audit rows are written for every event that passes the permission check, mirroring
    // what `apply_cdc_events` will itself accept (LWW may still no-op the actual table write,
    // but the audit trail records the change as received/authorized).
    for event in &events {
        let node_id_hex = event.node_id().unwrap_or("");
        let entity = syntrix_network::schema::resolve_table(&event.table)
            .map(|t| t.entity_name())
            .unwrap_or(event.table.as_str());
        if !perm(node_id_hex, entity) {
            continue;
        }
        write_audit_row(db, org_id, event);
    }

    if let Err(e) = syntrix_network::cdc::apply_cdc_events(db, &events, &perm) {
        tracing::warn!(target: "syntrix", org = %org_id, error = %e, "admin apply_cdc_batch: failed to apply events");
    }
}

fn write_audit_row(db: &Arc<turso_core::Connection>, org_id: &str, event: &CdcEvent) {
    let entity = syntrix_network::schema::resolve_table(&event.table)
        .map(|t| t.entity_name())
        .unwrap_or(event.table.as_str());
    let change_type = match event.change_type {
        1 => "insert",
        0 => "update",
        -1 => "delete",
        _ => "write",
    };
    let doc_id = event
        .get_str("doc_id")
        .or_else(|| event.get_str("line_id"))
        .unwrap_or("")
        .to_string();
    let node_id = event.node_id().unwrap_or("").to_string();
    let row_image_str = serde_json::to_string(&event.columns).unwrap_or_default();

    let mut stmt = match db.prepare(
        "INSERT INTO event_log (org_id, entity, change_type, doc_id, row_image, change_time, node_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[admin-gossip] failed to write CDC audit row for {entity}: {e}");
            return;
        }
    };

    let binds: [(usize, turso_core::Value); 7] = [
        (1, turso_core::Value::from_text(org_id.to_string())),
        (2, turso_core::Value::from_text(entity.to_string())),
        (3, turso_core::Value::from_text(change_type.to_string())),
        (4, turso_core::Value::from_text(doc_id)),
        (5, turso_core::Value::from_text(row_image_str)),
        (6, turso_core::Value::from_i64(event.change_time)),
        (7, turso_core::Value::from_text(node_id)),
    ];
    for (idx, value) in binds {
        if let Err(e) = stmt.bind_at(NonZero::new(idx).unwrap(), value) {
            eprintln!("[admin-gossip] failed to bind CDC audit row for {entity}: {e}");
            return;
        }
    }

    loop {
        match stmt.step() {
            Ok(turso_core::StepResult::Row) => continue,
            Ok(turso_core::StepResult::Done) => break,
            Ok(turso_core::StepResult::IO | turso_core::StepResult::Yield) => {
                if let Err(e) = stmt._io().step() {
                    eprintln!("[admin-gossip] failed to write CDC audit row for {entity}: {e}");
                    return;
                }
            }
            Ok(turso_core::StepResult::Interrupt | turso_core::StepResult::Busy) => continue,
            Err(e) => {
                eprintln!("[admin-gossip] failed to write CDC audit row for {entity}: {e}");
                return;
            }
        }
    }
}
