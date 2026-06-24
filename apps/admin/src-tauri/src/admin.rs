use futures_util::StreamExt;
use iroh_docs::api::protocol::{ShareMode, AddrInfoOptions};

use crate::identity::AppState;
use crate::{DeviceInfo, RoleInfo};

pub async fn create_org(state: &mut AppState, name: &str) -> anyhow::Result<()> {
    let api = state.api().clone();
    let author = state.author();

    let control_doc = api.create().await?;
    let catalogs_doc = api.create().await?;
    let operational_doc = api.create().await?;
    let payroll_doc = api.create().await?;

    let node_id_hex = hex::encode(state.node_id());
    let device_json = serde_json::json!({
        "active": true, "role": "admin", "person": "admin",
        "name": format!("Admin ({})", name),
    });
    control_doc.set_bytes(
        author, format!("members/{}", node_id_hex).into_bytes(),
        serde_json::to_vec(&device_json)?,
    ).await?;

    let admin_role = serde_json::json!({"can_open": ["control","catalogs","operational","payroll"], "can_write": ["catalogs","operational","payroll"]});
    control_doc.set_bytes(author, b"roles/admin".to_vec(), serde_json::to_vec(&admin_role)?).await?;

    let org_json = serde_json::json!({"name": name, "created_at": chrono::Utc::now().to_rfc3339()});
    control_doc.set_bytes(author, b"org".to_vec(), serde_json::to_vec(&org_json)?).await?;

    // Register namespaces for accept_cb
    if let Ok(mut reg) = state.registry().write() {
        reg.map_namespace_to_org(control_doc.id(), name.into());
        reg.map_namespace_to_org(catalogs_doc.id(), name.into());
        reg.map_namespace_to_org(operational_doc.id(), name.into());
        reg.map_namespace_to_org(payroll_doc.id(), name.into());
    }

    state.remember_device(name, &node_id_hex, "admin", "admin", &format!("Admin ({})", name), true, "");
    state.add_org(name, control_doc.clone(), catalogs_doc, operational_doc, payroll_doc);
    
    start_heartbeat(control_doc, author, node_id_hex);

    Ok(())
}

pub async fn add_device(
    state: &mut AppState, org: &str, node_id: &str, name: &str, person: &str, role: &str, device_addr: &str,
) -> anyhow::Result<()> {
    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;
    let doc = &org_state.control_doc;
    let author = state.author();

    let device_json = serde_json::json!({"active": true, "role": role, "person": person, "name": name});
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
        "admin" => serde_json::json!({"can_open": ["customers","suppliers","products","invoices","orders"], "can_write": ["customers","suppliers","products","invoices","orders"]}),
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

/// List devices from in-memory cache.
pub async fn list_devices(state: &mut AppState, org: &str) -> anyhow::Result<Vec<DeviceInfo>> {
    Ok(state.list_org_devices(org))
}

/// List roles from in-memory cache (populated during add_device/create_org).
pub async fn list_roles(state: &mut AppState, org: &str) -> anyhow::Result<Vec<RoleInfo>> {
    let roles = state.list_org_roles(org);
    Ok(roles)
}

pub fn network_status(_state: &AppState) -> String {
    "online (iroh P2P node running)".into()
}

/// Generate tickets for sharing an org's control + data docs.
pub async fn share_org_tickets(state: &mut AppState, org: &str) -> anyhow::Result<Vec<String>> {
    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;

    let control_ticket = org_state.control_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
    let catalogs_ticket = org_state.catalogs_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
    let operational_ticket = org_state.operational_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
    let payroll_ticket = org_state.payroll_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;

    Ok(vec![
        control_ticket.to_string(),
        catalogs_ticket.to_string(),
        operational_ticket.to_string(),
        payroll_ticket.to_string(),
    ])
}

/// Send an org invitation to a client device.
/// Accepts either a JSON with node_id + addrs, or just a hex node_id.
pub async fn send_invite(
    state: &mut AppState,
    org: &str,
    endpoint_addr_json: &str,
    role: &str,
    name: &str,
    person: &str,
) -> anyhow::Result<()> {
    // Try parsing as JSON (full address), fall back to raw hex node_id
    let (peer, addrs, _addr) = if let Ok(addr_data) = serde_json::from_str::<serde_json::Value>(endpoint_addr_json) {
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
                    // Include relay URL for relay-based discovery
                    relay_str.parse::<iroh::RelayUrl>().ok().map(iroh::TransportAddr::Relay)
                } else {
                    let addr_str = s.strip_prefix("ip:").unwrap_or(s);
                    addr_str.parse::<std::net::SocketAddr>().ok().map(iroh::TransportAddr::Ip)
                }
            }).collect())
            .unwrap_or_default();
        let addr = iroh::EndpointAddr::from_parts(peer, addrs.clone());
        (peer, addrs, addr)
    } else {
        // Raw hex node_id — rely on DNS
        let node_id_bytes = hex::decode(endpoint_addr_json)?;
        let node_id: [u8; 32] = node_id_bytes.as_slice().try_into()
            .map_err(|_| anyhow::anyhow!("invalid node_id length"))?;
        let peer: iroh::PublicKey = iroh::PublicKey::from_bytes(&node_id)?;
        let addr = iroh::EndpointAddr::from_parts(peer, []);
        (peer, vec![], addr)
    };

    let org_state = state.get_org(org)
        .ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;

    let control_ticket = org_state.control_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
    let catalogs_ticket = org_state.catalogs_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
    let operational_ticket = org_state.operational_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
    let _payroll_ticket = org_state.payroll_doc
        .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;

    // Build selective ticket list based on business modules -> namespaces mapping
    let grants = default_role_grants(role);
    let can_open: Vec<&str> = grants["can_open"].as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let needs_control = true; // Everyone needs control docs
    let mut needs_catalogs = false;
    let mut needs_operational = false;

    for perm in &can_open {
        match *perm {
            "products" | "suppliers" => needs_catalogs = true,
            "customers" | "invoices" | "orders" => needs_operational = true,
            _ => {}
        }
    }

    let mut tickets: Vec<serde_json::Value> = vec![];
    if needs_control { tickets.push(serde_json::json!({"ns": "control", "ticket": control_ticket.to_string()})); }
    if needs_catalogs { tickets.push(serde_json::json!({"ns": "catalogs", "ticket": catalogs_ticket.to_string()})); }
    if needs_operational { tickets.push(serde_json::json!({"ns": "operational", "ticket": operational_ticket.to_string()})); }

    let payload = serde_json::json!({
        "org_name": org,
        "role": role,
        "tickets": tickets,
    });

    let endpoint = state.endpoint();
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
    // Wait for client to read before closing connection (race condition fix)
    let _ = conn.closed().await;

    // Register the device locally now that the invite was sent
    let node_id_hex = hex::encode(&peer.as_bytes()[..]);
    add_device(state, org, &node_id_hex, name, person, role, endpoint_addr_json).await?;

    Ok(())
}

pub async fn create_role(
    state: &mut AppState, org: &str, name: &str, can_open: Vec<String>, can_write: Vec<String>,
) -> anyhow::Result<()> {
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

    // Update in-memory
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

    // Apply partial changes
    if let Some(obj) = role_json.as_object_mut() {
        for (k, v) in changes {
            obj.insert(k, v);
        }
    }

    doc.set_bytes(author, role_key.into_bytes(), serde_json::to_vec(&role_json)?).await?;

    // Update in-memory state
    let can_open = role_json["can_open"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let can_write = role_json["can_write"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    state.set_role(org, key, can_open, can_write);
    
    Ok(())
}

pub fn start_heartbeat(doc: iroh_docs::api::Doc, author: iroh_docs::AuthorId, node_id: String) {
    tokio::spawn(async move {
        loop {
            let ts = chrono::Utc::now().timestamp_millis();
            let key = format!("heartbeat/{}", node_id);
            let val = serde_json::json!({"ts": ts, "status": "online"});
            let _ = doc.set_bytes(author, key.into_bytes(), serde_json::to_vec(&val).unwrap()).await;
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}

#[derive(serde::Serialize)]
pub struct PeerStatus {
    pub node_id: String,
    pub status: String,
    pub last_seen: i64,
}

#[derive(serde::Serialize)]
pub struct SyncInfo {
    pub node_id: String,
    pub is_online: bool,
    pub peers: Vec<PeerStatus>,
}

pub async fn get_sync_info(state: &AppState, org: &str) -> anyhow::Result<SyncInfo> {
    let org_state = state.get_org(org).ok_or_else(|| anyhow::anyhow!("org {} not found", org))?;
    let doc = &org_state.control_doc;
    
    let mut entries = Box::pin(doc.get_many(iroh_docs::store::Query::key_prefix("heartbeat/")).await?);
    let mut peers = Vec::new();
    let now = chrono::Utc::now().timestamp_millis();
    
    while let Some(res) = entries.next().await {
        let entry = res?;
        let key_bytes = entry.key();
        if let Ok(key) = std::str::from_utf8(key_bytes) {
            if let Some(peer_id) = key.strip_prefix("heartbeat/") {
                if let Ok(bytes) = state.store().blobs().get_bytes(entry.content_hash()).await {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(ts) = val["ts"].as_i64() {
                            let status = if now - ts < 60000 { "online" } else { "offline" };
                            peers.push(PeerStatus {
                                node_id: peer_id.to_string(),
                                status: status.to_string(),
                                last_seen: ts,
                            });
                        }
                    }
                }
            }
        }
    }
    
    Ok(SyncInfo {
        node_id: hex::encode(state.node_id()),
        is_online: true,
        peers,
    })
}
