
use std::collections::HashMap;
use iroh_docs::api::protocol::{ShareMode, AddrInfoOptions};

use crate::identity::AppState;
use crate::{DeviceInfo, RoleInfo};
pub use syntrix_core::{build_device_addr_string, start_heartbeat_with_resync, SyncInfo, get_sync_info};
use syntrix_schema::all_schemas;


pub async fn create_org(state: &mut AppState, name: &str) -> anyhow::Result<()> {
    let api = state.api().clone();
    let author = state.author();

    let control_doc = api.create().await?;
    let mut entity_docs = HashMap::new();
    for schema in all_schemas() {
        let doc = api.create().await?;
        entity_docs.insert(schema.name, doc);
    }

    let node_id_hex = hex::encode(state.node_id());
    let own_device_addr = build_device_addr_string(state.endpoint());
    let device_json = serde_json::json!({
        "active": true,
        "role": "admin",
        "person": "admin",
        "name": format!("Admin ({})", name),
        "device_addr": own_device_addr,
    });
    control_doc.set_bytes(
        author, format!("members/{}", node_id_hex).into_bytes(),
        serde_json::to_vec(&device_json)?,
    ).await?;

    let admin_role = serde_json::json!({"can_open": ["*"], "can_write": ["*"]});
    control_doc.set_bytes(author, b"roles/admin".to_vec(), serde_json::to_vec(&admin_role)?).await?;

    let org_json = serde_json::json!({"name": name, "created_at": chrono::Utc::now().to_rfc3339()});
    control_doc.set_bytes(author, b"org".to_vec(), serde_json::to_vec(&org_json)?).await?;

    // Register namespaces for accept_cb
    if let Ok(mut reg) = state.registry().write() {
        reg.map_namespace_to_org(control_doc.id(), name.into());
        for (_ns, doc) in &entity_docs {
            reg.map_namespace_to_org(doc.id(), name.into());
        }
    }

    let ctrl_id = control_doc.id().to_string();
    let namespace_ids: HashMap<String, String> = entity_docs.iter().map(|(k, v)| (k.clone(), v.id().to_string())).collect();

    state.remember_device(name, &node_id_hex, "admin", "admin", &format!("Admin ({})", name), true, &own_device_addr);
    state.add_org(name, control_doc.clone(), entity_docs.clone());
    state.save_org_config(name, &ctrl_id, &namespace_ids)?;
    
    let store = state.store().clone();
    let secret = state.secret().clone();
    let entity_docs_vec: Vec<iroh_docs::api::Doc> = entity_docs.into_values().collect();
    start_heartbeat_with_resync(
        control_doc.clone(), entity_docs_vec,
        author, node_id_hex, store, secret, state.registry().clone(),
    );

    Ok(())
}

pub async fn add_device(
    state: &mut AppState, org: &str, node_id: &str, name: &str, person: &str, role: &str, device_addr: &str,
) -> anyhow::Result<()> {
    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;
    let doc = &org_state.control_doc;
    let author = state.author();

    let device_json = serde_json::json!({
        "active": true,
        "role": role,
        "person": person,
        "name": name,
        "device_addr": device_addr,
    });
    doc.set_bytes(author, format!("members/{}", node_id).into_bytes(),
        serde_json::to_vec(&device_json)?,
    ).await?;

    let role_key = format!("roles/{}", role);
    let existing = doc.get_exact(author, role_key.as_bytes(), false).await?;
    if existing.is_none() {
        let grants = default_role_grants(role);
        doc.set_bytes(author, role_key.into_bytes(), serde_json::to_vec(&grants)?).await?;
    }

    state.remember_device(org, node_id, role, person, name, true, device_addr);
    Ok(())
}

fn default_role_grants(role: &str) -> serde_json::Value {
    match role {
        "admin" => serde_json::json!({"can_open": ["*"], "can_write": ["*"]}),
        "sales" => serde_json::json!({"can_open": ["customers","products","invoices","orders"], "can_write": ["customers","invoices","orders"]}),
        "contabilidad" => serde_json::json!({"can_open": ["invoices","customers"], "can_write": []}),
        _ => serde_json::json!({"can_open": [], "can_write": []}),
    }
}

pub async fn update_device(
    state: &mut AppState, org: &str, node_id: &str, active: bool, role: Option<String>, name: Option<String>, person: Option<String>,
) -> anyhow::Result<()> {
    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;
    let doc = &org_state.control_doc;
    let author = state.author();

    let key = format!("members/{}", node_id);
    let entry = doc.get_exact(author, key.as_bytes(), false).await?;
    let mut value = if let Some(e) = entry {
        let bytes = state.store().blobs().get_bytes(e.content_hash()).await?;
        serde_json::from_slice(&bytes).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    value["active"] = serde_json::Value::Bool(active);
    if let Some(r) = role.clone() { value["role"] = serde_json::Value::String(r); }
    if let Some(n) = name.clone() { value["name"] = serde_json::Value::String(n); }
    if let Some(p) = person.clone() { value["person"] = serde_json::Value::String(p); }

    doc.set_bytes(author, key.into_bytes(), serde_json::to_vec(&value)?).await?;

    state.update_device(org, node_id, active, role, name, person);
    Ok(())
}

pub async fn list_devices(state: &mut AppState, org: &str) -> anyhow::Result<Vec<DeviceInfo>> {
    Ok(state.list_org_devices(org))
}

pub async fn list_roles(state: &mut AppState, org: &str) -> anyhow::Result<Vec<RoleInfo>> {
    let roles = state.list_org_roles(org);
    Ok(roles)
}

pub fn network_status(_state: &AppState) -> String {
    "online (iroh P2P node running)".into()
}

pub async fn share_org_tickets(state: &mut AppState, org: &str) -> anyhow::Result<Vec<String>> {
    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;

    let control_ticket = org_state.control_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;

    let mut tickets = vec![control_ticket.to_string()];
    for (_ns, doc) in &org_state.entity_docs {
        let ticket = doc.share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
        tickets.push(ticket.to_string());
    }

    Ok(tickets)
}

/// Send an org invitation to a client device.
pub async fn send_invite(
    control_doc: iroh_docs::api::Doc,
    entity_docs: HashMap<String, iroh_docs::api::Doc>,
    endpoint: iroh::Endpoint,
    org: &str,
    endpoint_addr_json: &str,
    role: &str,
) -> anyhow::Result<(String, String)> {
    let (peer, addrs, device_addr_str) = if let Ok(addr_data) = serde_json::from_str::<serde_json::Value>(endpoint_addr_json) {
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

    let control_ticket = control_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;

    // Determine which entity docs to share based on role's can_open
    let grants = default_role_grants(role);
    let can_open: Vec<String> = grants["can_open"].as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut tickets: Vec<serde_json::Value> = vec![];
    tickets.push(serde_json::json!({"ns": "control", "ticket": control_ticket.to_string()}));

    if can_open.iter().any(|e| e == "*") {
        // All entities → include all entity doc tickets
        for (ns_name, doc) in &entity_docs {
            let ticket = doc.share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
            tickets.push(serde_json::json!({"ns": ns_name, "ticket": ticket.to_string()}));
        }
    } else {
        for ns_name in &can_open {
            if let Some(doc) = entity_docs.get(ns_name) {
                let ticket = doc.share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
                tickets.push(serde_json::json!({"ns": ns_name, "ticket": ticket.to_string()}));
            }
        }
    }

    let payload = serde_json::json!({
        "org_name": org,
        "role": role,
        "admin_addr": build_device_addr_string(&endpoint),
        "tickets": tickets,
    });

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

    let node_id_hex = hex::encode(&peer.as_bytes()[..]);
    Ok((node_id_hex, device_addr_str))
}

/// Validate that every string in a permission list is a known entity name or "*".
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

    let org_state = state.get_org(org).ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;
    let doc = &org_state.control_doc;
    let author = state.author();

    let role_key = format!("roles/{}", name);
    let existing = doc.get_exact(author, role_key.as_bytes(), false).await?;
    if existing.is_some() {
        return Err(anyhow::anyhow!("El rol {} ya existe", name));
    }

    let grants = serde_json::json!({
        "can_open": can_open,
        "can_write": can_write,
    });
    doc.set_bytes(author, role_key.into_bytes(), serde_json::to_vec(&grants)?).await?;

    state.set_role(org, name, can_open, can_write);
    Ok(())
}

pub async fn update_role(
    state: &mut AppState, org: &str, key: &str, changes: std::collections::HashMap<String, serde_json::Value>,
) -> anyhow::Result<()> {
    let org_state = state.get_org(org).ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;
    let doc = &org_state.control_doc;
    let author = state.author();

    let role_key = format!("roles/{}", key);
    let entry = doc.get_exact(author, role_key.as_bytes(), false).await?
        .ok_or_else(|| anyhow::anyhow!("Role {} not found", key))?;
    let bytes = state.store().blobs().get_bytes(entry.content_hash()).await?;
    let mut role_json: serde_json::Value = serde_json::from_slice(&bytes)?;

    if let Some(obj) = role_json.as_object_mut() {
        for (k, v) in changes {
            obj.insert(k, v);
        }
    }

    doc.set_bytes(author, role_key.into_bytes(), serde_json::to_vec(&role_json)?).await?;

    let can_open = role_json["can_open"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let can_write = role_json["can_write"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    state.set_role(org, key, can_open, can_write);
    
    Ok(())
}
