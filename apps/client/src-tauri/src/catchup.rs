use std::sync::Arc;

use iroh::{Endpoint, endpoint::Connection};
use iroh::protocol::ProtocolHandler;
use tokio::io::AsyncWriteExt;

use crate::indexes::RelationalEngine;

pub const CATCHUP_ALPN: &[u8] = b"/syntrix/catchup/1";

pub struct CatchupProtocol {
    indexer: Arc<RelationalEngine>,
}

impl std::fmt::Debug for CatchupProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CatchupProtocol").finish_non_exhaustive()
    }
}

impl CatchupProtocol {
    pub fn new(indexer: Arc<RelationalEngine>) -> Self {
        Self { indexer }
    }

    pub async fn request_catchup(
        endpoint: &Endpoint,
        admin_addr: &str,
        org_id: &str,
        since_hlc: u64,
        indexer: &RelationalEngine,
    ) -> anyhow::Result<()> {
        let admin_endpoint = syntrix_core::parse_device_addr(admin_addr)
            .ok_or_else(|| anyhow::anyhow!("invalid admin addr"))?;
        let conn = endpoint.connect(admin_endpoint, CATCHUP_ALPN).await?;

        let request = serde_json::json!({
            "org_id": org_id,
            "since_hlc": since_hlc,
        });

        let mut send = conn.open_uni().await?;
        send.write_all(serde_json::to_vec(&request)?.as_slice()).await?;
        send.finish()?;

        let mut recv = conn.accept_uni().await?;
        let buf = recv.read_to_end(65536).await?;

        let events: Vec<serde_json::Value> = serde_json::from_slice(&buf)?;
        for event in events {
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
}

impl ProtocolHandler for CatchupProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl std::future::Future<Output = Result<(), iroh::protocol::AcceptError>> + Send {
        let indexer = self.indexer.clone();
        async move {
            let mut recv = connection.accept_uni().await.map_err(|e| {
                iroh::protocol::AcceptError::from_boxed(Box::new(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("accept_uni: {e}"))
                ))
            })?;

            let buf = recv.read_to_end(65536).await.map_err(|e| {
                iroh::protocol::AcceptError::from_boxed(Box::new(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("read_to_end: {e}"))
                ))
            })?;

            let request: serde_json::Value = serde_json::from_slice(&buf).map_err(|e| {
                iroh::protocol::AcceptError::from_boxed(Box::new(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("parse: {e}"))
                ))
            })?;

            let org_id = request["org_id"].as_str().unwrap_or("");
            let since_hlc = request["since_hlc"].as_u64().unwrap_or(0);

            if let Ok(events) = indexer.query_events_since(org_id, since_hlc, 10000) {
                let event_data: Vec<serde_json::Value> = events.into_iter().map(|e| {
                    serde_json::json!({
                        "type": e.event_type,
                        "hlc": e.hlc,
                        "schema_version": e.schema_version,
                        "payload": e.payload,
                    })
                }).collect();

                let bytes = serde_json::to_vec(&event_data).map_err(|e| {
                    iroh::protocol::AcceptError::from_boxed(Box::new(
                        std::io::Error::new(std::io::ErrorKind::Other, format!("serialize: {e}"))
                    ))
                })?;

                let mut send = connection.open_uni().await.map_err(|e| {
                    iroh::protocol::AcceptError::from_boxed(Box::new(
                        std::io::Error::new(std::io::ErrorKind::Other, format!("open_uni: {e}")
                    )))
                })?;

                send.write_all(&bytes).await.map_err(|e| {
                    iroh::protocol::AcceptError::from_boxed(Box::new(
                        std::io::Error::new(std::io::ErrorKind::Other, format!("write: {e}"))
                    ))
                })?;
                send.finish().map_err(|e| {
                    iroh::protocol::AcceptError::from_boxed(Box::new(
                        std::io::Error::new(std::io::ErrorKind::Other, format!("finish: {e}"))
                    ))
                })?;
            }

            Ok(())
        }
    }
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
