use std::sync::Arc;

pub fn start_heartbeat_with_resync(
    node_id: String,
    broadcast: Arc<dyn Fn(&str) + Send + Sync>,
    store_heartbeat: Arc<dyn Fn(i64, &str) + Send + Sync>,
) {
    tokio::spawn(async move {
        loop {
            let ts = chrono::Utc::now().timestamp_millis();
            let val = serde_json::json!({"ts": ts, "status": "online", "node_id": node_id});
            let json_str = serde_json::to_string(&val).unwrap();
            broadcast(&json_str);
            store_heartbeat(ts, &node_id);
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}
