use std::sync::Arc;

use iroh::endpoint::Connection;
use iroh::protocol::ProtocolHandler;
pub const CATCHUP_ALPN: &[u8] = b"/syntrix/catchup/1";

#[derive(Debug)]
pub struct AdminCatchupProtocol {
    db: Arc<redb::Database>,
}

const EVENT_LOG: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("event_log");

impl AdminCatchupProtocol {
    pub fn new(db: Arc<redb::Database>) -> Self {
        Self { db }
    }
}

impl ProtocolHandler for AdminCatchupProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl std::future::Future<Output = Result<(), iroh::protocol::AcceptError>> + Send {
        let db = self.db.clone();
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

            let events = query_events_since(&db, org_id, since_hlc, 10000).unwrap_or_default();

            let bytes = serde_json::to_vec(&events).map_err(|e| {
                iroh::protocol::AcceptError::from_boxed(Box::new(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("serialize: {e}"))
                ))
            })?;

            let mut send = connection.open_uni().await.map_err(|e| {
                iroh::protocol::AcceptError::from_boxed(Box::new(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("open_uni: {e}"))
                ))
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

            Ok(())
        }
    }
}

fn query_events_since(
    db: &redb::Database,
    org_id: &str,
    cursor_ts: u64,
    limit: usize,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let read_txn = db.begin_read()?;
    let event_log = read_txn.open_table(EVENT_LOG)?;

    let prefix = format!("evt:{}:", org_id);
    let range = event_log.range(prefix.as_str()..)?;

    let mut results = Vec::new();
    for item in range {
        let (key, value) = item?;
        let k = key.value();
        if !k.starts_with(&prefix) { break; }

        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(value.value()) {
            let hlc_ts = val["hlc"]["ts"].as_u64().unwrap_or(0);
            if hlc_ts <= cursor_ts { continue; }
            results.push(val);
        }

        if results.len() >= limit { break; }
    }

    Ok(results)
}
