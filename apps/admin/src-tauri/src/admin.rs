use crate::identity::AppState;
use crate::{DeviceInfo, RoleInfo};
use syntrix_network::codecs::InvitePayload;
pub use syntrix_core::{build_device_addr_string, SyncInfo};
use syntrix_core::ENTITY_NAMES;
use std::collections::HashMap;
use std::str::FromStr;


pub async fn create_org(state: &mut AppState, name: &str) -> anyhow::Result<()> {
    tracing::info!(org = %name, op = "create_org", step = "init", "generating topic id");

    let topic_id_str = format!("syntrix-org-{}", name);
    let node_id_hex = hex::encode(state.node_id());
    let peer_id = state.p2p().local_peer_id();
    let addrs = state.p2p().listen_addrs().await;
    let own_device_addr = syntrix_core::build_device_addr_string(peer_id, &addrs);

    if let Ok(mut reg) = state.registry().write() {
        reg.set_topic_id(name.into(), topic_id_str.clone());
    }

    state.remember_device(name, &node_id_hex, "admin", "admin", &format!("Admin ({})", name), true, &own_device_addr);
    state.add_org(name, topic_id_str.clone());
    state.save_org_config(name, &topic_id_str)?;

    tracing::info!(org = %name, op = "create_org", step = "save", "org config saved");

    state.join_gossip_and_heartbeat(name, topic_id_str).await?;

    tracing::info!(org = %name, op = "create_org", step = "done", node_id = %node_id_hex, "completed, gossip topic joined");
    Ok(())
}

pub async fn add_device(
    state: &mut AppState, org: &str, node_id: &str, name: &str, person: &str, role: &str, device_addr: &str,
) -> anyhow::Result<()> {
    state.remember_device(org, node_id, role, person, name, true, device_addr);
    tracing::info!(org = %org, op = "add_device", step = "register", node_id = %node_id, role = %role, name = %name, "device registered");

    // Publish device.updated so all existing peers learn about the new
    // device and can validate its future writes (Fase 3 CDC permission
    // checks). Without this, peers that joined before this device was
    // added will reject CDC batches authored by it.
    let event = serde_json::json!({
        "type": "device.updated",
        "ts": chrono::Utc::now().timestamp_millis(),
        "payload": {
            "org": org,
            "node_id": node_id,
            "active": true,
            "role": role,
            "person": person,
            "name": name,
        },
    });
    let topic = format!("syntrix-org-{}", org);
    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = state.p2p().publish(&topic, bytes);
    }

    Ok(())
}

pub async fn update_device(
    state: &mut AppState, org: &str, node_id: &str, active: bool, role: Option<String>, name: Option<String>, person: Option<String>, device_type: Option<String>,
) -> anyhow::Result<()> {
    let effective_role = if role.is_some() {
        role.clone()
    } else {
        state.list_org_devices(org).into_iter()
            .find(|d| d.node_id == node_id)
            .map(|d| d.role)
    };
    state.update_device(org, node_id, active, role, name, person, device_type);

    let event = serde_json::json!({
        "type": "device.updated",
        "ts": chrono::Utc::now().timestamp_millis(),
        "payload": {
            "org": org,
            "node_id": node_id,
            "active": active,
            "role": effective_role.unwrap_or_default(),
        },
    });
    let topic = format!("syntrix-org-{}", org);
    if let Ok(bytes) = serde_json::to_vec(&event) {
        if let Err(e) = state.p2p().publish(&topic, bytes) {
            tracing::warn!(target: "syntrix_admin_lib::admin", org = %org, error = %e, "update_device: publish failed");
        }
    }

    // Block peer at network level when device is deactivated
    if !active {
        if let Some(device) = state.list_org_devices(org).into_iter().find(|d| d.node_id == node_id) {
            if let Some(peer_id) = syntrix_core::parse_device_addr(&device.device_addr) {
                state.p2p().block_peer(peer_id);
                tracing::info!(target: "syntrix_admin_lib::admin", peer = %peer_id, "blocked deactivated device");
            }
        }
    }

    Ok(())
}

pub async fn list_devices(state: &mut AppState, org: &str) -> anyhow::Result<Vec<DeviceInfo>> {
    Ok(state.list_org_devices(org))
}

pub async fn list_roles(state: &mut AppState, org: &str) -> anyhow::Result<Vec<RoleInfo>> {
    Ok(state.list_org_roles(org))
}

pub fn cdc_health(state: &AppState) -> serde_json::Value {
    let event_count = state.list_org_devices("*").len() as i64; // dummy
    serde_json::json!({
        "pending_events": event_count,
        "events_logged": 0i64,
        "status": "ok",
    })
}

pub fn network_status(_state: &AppState) -> String {
    "online (libp2p P2P node running)".into()
}

pub async fn get_invite_info(state: &AppState, org: &str) -> anyhow::Result<serde_json::Value> {
    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;
    let peer_id = state.p2p().local_peer_id();
    let addrs = state.p2p().listen_addrs().await;

    Ok(serde_json::json!({
        "org_name": org,
        "topic_id": &org_state.topic_id,
        "admin_addr": syntrix_core::build_device_addr_string(peer_id, &addrs),
    }))
}

pub async fn send_invite(
    state: &AppState,
    org: &str,
    endpoint_addr_json: &str,
    role: &str,
    topic_id: String,
    admin_addr: String,
    can_open: Vec<String>,
    can_write: Vec<String>,
) -> anyhow::Result<()> {
    let peer_id = if let Ok(addr_data) = serde_json::from_str::<serde_json::Value>(endpoint_addr_json) {
        let pid_str = addr_data["peer_id"].as_str()
            .ok_or_else(|| anyhow::anyhow!("invalid addr json: missing peer_id"))?;
        libp2p::PeerId::from_str(pid_str)?
    } else {
        libp2p::PeerId::from_str(endpoint_addr_json)?
    };

    let payload = InvitePayload {
        org_name: org.to_string(),
        role: role.to_string(),
        admin_addr: Some(admin_addr),
        topic_id: topic_id.clone(),
        can_open: can_open.clone(),
        can_write: can_write.clone(),
    };

    tracing::info!(org = %org, op = "send_invite", step = "connect", role = %role, "sending invite to peer");

    state.p2p().send_invite(peer_id, payload)?;

    tracing::info!(org = %org, op = "send_invite", step = "done", role = %role, "invite delivered");
    Ok(())
}

fn validate_permissions(perms: &[String]) -> Result<(), String> {
    for p in perms {
        if p == "*" { continue; }
        if ENTITY_NAMES.contains(&p.as_str()) { continue; }
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

    state.set_role(org, name, can_open.clone(), can_write.clone());

    let event = serde_json::json!({
        "type": "role.updated",
        "ts": chrono::Utc::now().timestamp_millis(),
        "payload": {
            "org": org,
            "name": name,
            "can_open": can_open,
            "can_write": can_write,
        },
    });
    let topic = format!("syntrix-org-{}", org);
    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = state.p2p().publish(&topic, bytes);
    }

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

    state.set_role(org, key, can_open.clone(), can_write.clone());

    let event = serde_json::json!({
        "type": "role.updated",
        "ts": chrono::Utc::now().timestamp_millis(),
        "payload": {
            "org": org,
            "name": key,
            "can_open": can_open,
            "can_write": can_write,
        },
    });
    let topic = format!("syntrix-org-{}", org);
    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = state.p2p().publish(&topic, bytes);
    }

    Ok(())
}

pub fn get_endpoint_addr_impl(state: &AppState) -> String {
    let peer_id = state.p2p().local_peer_id();
    let addrs = tauri::async_runtime::block_on(state.p2p().listen_addrs());
    serde_json::json!({
        "node_id": hex::encode(state.node_id()),
        "peer_id": peer_id.to_base58(),
        "addrs": addrs,
    }).to_string()
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

/// Full invite flow: parse endpoint → add_device → dial → wait → send_invite.
/// Used by both the Tauri command wrapper and the headless test mode.
pub async fn send_invite_full(
    state: &mut AppState,
    org: &str,
    endpoint_addr_json: &str,
    role: &str,
    name: &str,
    person: &str,
) -> anyhow::Result<()> {
    // Parse endpoint
    let node_id_hex = if let Ok(addr_data) =
        serde_json::from_str::<serde_json::Value>(endpoint_addr_json)
    {
        addr_data["node_id"].as_str()
            .ok_or_else(|| anyhow::anyhow!("invalid endpoint: missing node_id"))?
            .to_string()
    } else {
        endpoint_addr_json.to_string()
    };

    // Register device
    add_device(state, org, &node_id_hex, name, person, role, endpoint_addr_json).await?;

    // Dial the client peer before sending the invite
    if let Ok(addr_data) = serde_json::from_str::<serde_json::Value>(endpoint_addr_json) {
        let peer_id_b58 = addr_data["peer_id"].as_str().unwrap_or("");
        if let Some(addrs) = addr_data["addrs"].as_array() {
            for addr_val in addrs {
                if let Some(addr_str) = addr_val.as_str() {
                    let with_p2p = format!("{}/p2p/{}", addr_str, peer_id_b58);
                    if let Ok(addr) = libp2p::Multiaddr::from_str(&with_p2p) {
                        tracing::info!(
                            target: "syntrix",
                            addr = %addr_str, peer = %peer_id_b58,
                            "dialing client peer before invite"
                        );
                        let _ = state.p2p().dial(addr);
                    }
                }
            }
        }
    }

    // Wait for QUIC handshake + identify
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Get role permissions
    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;
    let topic_id = org_state.topic_id.clone();
    let roles = state.list_org_roles(org);
    let (can_open, can_write) = roles.iter()
        .find(|r| r.name == role)
        .map(|r| (r.can_open.clone(), r.can_write.clone()))
        .unwrap_or_else(|| {
            let g = crate::identity::default_role_grants(role);
            (g.can_open, g.can_write)
        });

    // Get admin address
    let peer_id = state.p2p().local_peer_id();
    let addrs = state.p2p().listen_addrs().await;
    let admin_addr = build_device_addr_string(peer_id, &addrs);

    // Send invite via request-response
    send_invite(state, org, endpoint_addr_json, role, topic_id, admin_addr, can_open, can_write).await
}
