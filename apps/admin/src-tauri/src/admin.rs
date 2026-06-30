use crate::identity::AppState;
use crate::{DeviceInfo, RoleInfo};
pub use syntrix_core::{build_device_addr_string, SyncInfo};
use syntrix_schema::all_schemas;
use std::collections::HashMap;

pub async fn create_org(state: &mut AppState, name: &str) -> anyhow::Result<()> {
    tracing::info!(org = %name, op = "create_org", step = "init", "generating topic id");

    let topic_id_bytes: [u8; 32] = fastrand::u128(..).to_le_bytes().into_iter()
        .chain(fastrand::u128(..).to_le_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| anyhow::anyhow!("failed to generate topic id"))?;

    let topic_id = iroh_gossip::TopicId::from_bytes(topic_id_bytes);
    let node_id_hex = hex::encode(state.node_id());
    let own_device_addr = build_device_addr_string(state.endpoint());

    if let Ok(mut reg) = state.registry().write() {
        reg.set_topic_id(name.into(), topic_id);
    }

    state.remember_device(name, &node_id_hex, "admin", "admin", &format!("Admin ({})", name), true, &own_device_addr);
    state.add_org(name, topic_id_bytes);
    state.save_org_config(name, &hex::encode(topic_id_bytes))?;

    tracing::info!(org = %name, op = "create_org", step = "save", "org config saved");

    // Join the gossip topic so the admin receives heartbeats and broadcasts its own.
    state.join_gossip_and_heartbeat(name, topic_id_bytes).await?;

    tracing::info!(org = %name, op = "create_org", step = "done", node_id = %node_id_hex, "completed, gossip topic joined");
    Ok(())
}

pub async fn add_device(
    state: &mut AppState, org: &str, node_id: &str, name: &str, person: &str, role: &str, device_addr: &str,
) -> anyhow::Result<()> {
    state.remember_device(org, node_id, role, person, name, true, device_addr);
    tracing::info!(org = %org, op = "add_device", step = "register", node_id = %node_id, role = %role, name = %name, "device registered");
    Ok(())
}

pub async fn update_device(
    state: &mut AppState, org: &str, node_id: &str, active: bool, role: Option<String>, name: Option<String>, person: Option<String>,
) -> anyhow::Result<()> {
    state.update_device(org, node_id, active, role, name, person);
    Ok(())
}

pub async fn list_devices(state: &mut AppState, org: &str) -> anyhow::Result<Vec<DeviceInfo>> {
    Ok(state.list_org_devices(org))
}

pub async fn list_roles(state: &mut AppState, org: &str) -> anyhow::Result<Vec<RoleInfo>> {
    Ok(state.list_org_roles(org))
}

pub fn network_status(_state: &AppState) -> String {
    "online (iroh P2P node running)".into()
}

pub async fn get_invite_info(state: &AppState, org: &str) -> anyhow::Result<serde_json::Value> {
    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;

    Ok(serde_json::json!({
        "org_name": org,
        "topic_id": hex::encode(org_state.topic_id),
        "admin_addr": build_device_addr_string(state.endpoint()),
    }))
}

pub async fn send_invite(
    endpoint: iroh::Endpoint,
    org: &str,
    endpoint_addr_json: &str,
    role: &str,
    topic_id: [u8; 32],
    admin_addr: String,
    can_open: Vec<String>,
    can_write: Vec<String>,
) -> anyhow::Result<()> {
    let (peer, addrs, _) = if let Ok(addr_data) = serde_json::from_str::<serde_json::Value>(endpoint_addr_json) {
        let node_id_hex = addr_data["node_id"].as_str()
            .ok_or_else(|| anyhow::anyhow!("invalid addr json: missing node_id"))?;
        let node_id_bytes = hex::decode(node_id_hex)?;
        let node_id: [u8; 32] = node_id_bytes.as_slice().try_into()
            .map_err(|_| anyhow::anyhow!("invalid node_id length"))?;
        let peer: iroh::PublicKey = iroh::PublicKey::from_bytes(&node_id)?;
        let addrs: Vec<iroh::TransportAddr> = addr_data["addrs"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| {
                let s = v.as_str()?;
                if let Some(relay_str) = s.strip_prefix("relay:") {
                    relay_str.parse::<iroh::RelayUrl>().ok().map(iroh::TransportAddr::Relay)
                } else {
                    let addr_str = s.strip_prefix("ip:").unwrap_or(s);
                    addr_str.parse::<std::net::SocketAddr>().ok().map(iroh::TransportAddr::Ip)
                }
            }).collect())
            .unwrap_or_default();
        (peer, addrs, endpoint_addr_json.to_string())
    } else {
        let node_id_bytes = hex::decode(endpoint_addr_json)?;
        let node_id: [u8; 32] = node_id_bytes.as_slice().try_into()
            .map_err(|_| anyhow::anyhow!("invalid node_id length"))?;
        let peer: iroh::PublicKey = iroh::PublicKey::from_bytes(&node_id)?;
        (peer, vec![], endpoint_addr_json.to_string())
    };

    let payload = serde_json::json!({
        "org_name": org,
        "role": role,
        "admin_addr": admin_addr,
        "topic_id": hex::encode(topic_id),
        "can_open": can_open,
        "can_write": can_write,
    });

    tracing::info!(org = %org, op = "send_invite", step = "connect", role = %role, "connecting to peer");

    let conn = match endpoint.connect(peer, b"/syntrix/invite/1").await {
        Ok(c) => c,
        Err(_) => {
            let addr = iroh::EndpointAddr::from_parts(peer, addrs);
            endpoint.connect(addr, b"/syntrix/invite/1").await.map_err(|_e| {
                anyhow::anyhow!("Could not reach this device. It may be offline, restarted (new ID), or already a member.")
            })?
        }
    };
    let mut send = conn.open_uni().await?;
    send.write_all(serde_json::to_vec(&payload)?.as_slice()).await?;
    send.finish()?;
    let _ = conn.closed().await;

    tracing::info!(org = %org, op = "send_invite", step = "done", role = %role, "invite delivered");
    Ok(())
}

fn validate_permissions(perms: &[String]) -> Result<(), String> {
    let schemas = all_schemas();
    for p in perms {
        if p == "*" { continue; }
        if schemas.iter().any(|s| s.name == *p) { continue; }
        return Err(format!("Permiso inválido: '{}' no es una entidad conocida ni '*'", p));
    }
    Ok(())
}

pub async fn create_role(
    state: &mut AppState, org: &str, name: &str, can_open: Vec<String>, can_write: Vec<String>,
) -> anyhow::Result<()> {
    validate_permissions(&can_open).map_err(|e| anyhow::anyhow!("{}", e))?;
    validate_permissions(&can_write).map_err(|e| anyhow::anyhow!("{}", e))?;

    if state.list_org_roles(org).iter().any(|r| r.name == name) {
        return Err(anyhow::anyhow!("El rol {} ya existe", name));
    }

    state.set_role(org, name, can_open, can_write);
    Ok(())
}

pub async fn update_role(
    state: &mut AppState, org: &str, key: &str, changes: std::collections::HashMap<String, serde_json::Value>,
) -> anyhow::Result<()> {
    let current = state.list_org_roles(org).into_iter().find(|r| r.name == key)
        .ok_or_else(|| anyhow::anyhow!("Role {} not found", key))?;

    let mut can_open = current.can_open.clone();
    let mut can_write = current.can_write.clone();

    if let Some(v) = changes.get("can_open").and_then(|v| v.as_array()) {
        can_open = v.iter().filter_map(|v| v.as_str().map(String::from)).collect();
    }
    if let Some(v) = changes.get("can_write").and_then(|v| v.as_array()) {
        can_write = v.iter().filter_map(|v| v.as_str().map(String::from)).collect();
    }

    state.set_role(org, key, can_open, can_write);
    Ok(())
}

pub fn get_sync_info(
    state: &AppState,
    org: &str,
) -> SyncInfo {
    let node_id = state.node_id();
    let members: Vec<serde_json::Value> = state.list_org_devices(org).into_iter().map(|d| {
        serde_json::json!({
            "node_id": d.node_id,
            "name": d.name,
            "person": d.person,
            "role": d.role,
            "device_addr": d.device_addr,
        })
    }).collect();
    let heartbeats: HashMap<String, i64> = state.get_heartbeats(org);
    syntrix_core::sync::get_sync_info(members, heartbeats, node_id)
}
