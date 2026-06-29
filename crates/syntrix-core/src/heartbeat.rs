use std::sync::Arc;

pub fn start_heartbeat(
    broadcast: Arc<dyn Fn(&str) + Send + Sync>,
    _node_id: String,
) {
    tokio::spawn(async move {
        loop {
            let ts = chrono::Utc::now().timestamp_millis();
            let val = serde_json::json!({"ts": ts, "status": "online"});
            broadcast(&serde_json::to_string(&val).unwrap());
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}

pub fn start_heartbeat_with_resync(
    node_id: String,
    _node_id_hex: String,
    _registry: Arc<std::sync::RwLock<crate::registry::NamespaceRegistry>>,
    _org_id: String,
    broadcast: Arc<dyn Fn(&str) + Send + Sync>,
) {
    tokio::spawn(async move {
        loop {
            let ts = chrono::Utc::now().timestamp_millis();
            let val = serde_json::json!({"ts": ts, "status": "online", "node_id": node_id});
            broadcast(&serde_json::to_string(&val).unwrap());
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}
