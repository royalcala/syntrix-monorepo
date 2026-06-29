#[derive(serde::Serialize)]
pub struct PeerStatus {
    pub node_id: String,
    pub status: String,
    pub last_seen: i64,
    pub name: Option<String>,
    pub person: Option<String>,
    pub role: Option<String>,
    pub device_addr: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SyncInfo {
    pub node_id: String,
    pub is_online: bool,
    pub peers: Vec<PeerStatus>,
}

pub fn get_sync_info(
    members: Vec<serde_json::Value>,
    heartbeats: std::collections::HashMap<String, i64>,
    node_id: [u8; 32],
) -> SyncInfo {
    let now = chrono::Utc::now().timestamp_millis();
    let mut peers = Vec::new();
    let mut processed = std::collections::HashSet::new();

    for member_val in members {
        let peer_id = member_val["node_id"].as_str().unwrap_or("").to_string();
        let ts = heartbeats.get(&peer_id).cloned().unwrap_or(0);
        let status = if ts > 0 && now - ts < 60_000 { "online" } else { "offline" };

        let name = member_val["name"].as_str().map(String::from);
        let person = member_val["person"].as_str().map(String::from);
        let role = member_val["role"].as_str().map(String::from);
        let device_addr = member_val["device_addr"].as_str().map(String::from);

        processed.insert(peer_id.clone());
        peers.push(PeerStatus {
            node_id: peer_id,
            status: status.to_string(),
            last_seen: ts,
            name,
            person,
            role,
            device_addr,
        });
    }

    for (peer_id, ts) in heartbeats {
        if !processed.contains(&peer_id) {
            let status = if now - ts < 60_000 { "online" } else { "offline" };
            peers.push(PeerStatus {
                node_id: peer_id,
                status: status.to_string(),
                last_seen: ts,
                name: None,
                person: None,
                role: None,
                device_addr: None,
            });
        }
    }

    SyncInfo {
        node_id: hex::encode(node_id),
        is_online: true,
        peers,
    }
}
