use std::sync::Arc;

use syntrix_network::P2PNode;
use crate::indexes::RelationalEngine;

/// Request catchup from a peer using the libp2p request_response protocol.
pub async fn request_catchup(
    p2p: &P2PNode,
    peer_id: libp2p::PeerId,
    org_id: &str,
    since_hlc: u64,
    indexer: &RelationalEngine,
) -> anyhow::Result<()> {
    let events = p2p.request_catchup(peer_id, org_id.to_string(), since_hlc).await?;
    for event in &events {
        let event_type = event["type"].as_str().unwrap_or("");
        let entity = entity_from_event_type(event_type);
        let payload = event.get("payload").cloned().unwrap_or_default();
        let doc_id = payload.get("id")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
            .unwrap_or("");

        if let Some(hlc_val) = event.get("hlc") {
            let hlc = crate::indexes::HlcTimestamp {
                ts: hlc_val.get("ts").and_then(|v| v.as_u64()).unwrap_or(0),
                count: hlc_val.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                node: hlc_val.get("node").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            };
            let _ = indexer.upsert_document_with_hlc(org_id, entity, doc_id, &payload, &hlc);
        } else {
            let _ = indexer.upsert_document(org_id, entity, doc_id, &payload);
        }
    }

    Ok(())
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
