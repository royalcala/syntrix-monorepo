use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use libp2p::identity::Keypair;
use libp2p::Multiaddr;
use syntrix_core::NodeId;
use syntrix_core::registry::{Device, NamespaceRegistry, RoleGrants};
use syntrix_network::{Event, P2PNode};
use crate::{DeviceInfo, RoleInfo};
use serde::{Deserialize, Serialize};

pub use syntrix_core::parse_device_addr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrgConfig {
    pub name: String,
    pub topic_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PersistedDevice {
    org: String,
    node_id: String,
    active: bool,
    role: String,
    person: String,
    name: String,
    device_addr: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PersistedRole {
    org: String,
    name: String,
    can_open: Vec<String>,
    can_write: Vec<String>,
}

pub struct AppState {
    keypair: Keypair,
    p2p: P2PNode,
    event_rx: tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Event>>>,
    heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>>,
    registry: Arc<RwLock<NamespaceRegistry>>,
    orgs: HashMap<String, OrgState>,
    devices: HashMap<String, HashMap<String, DeviceInfo>>,
    roles: HashMap<String, HashMap<String, RoleInfo>>,
    data_dir: PathBuf,
    pub db: Arc<turso_core::Connection>,
}

#[derive(Clone)]
pub struct OrgState {
    pub name: String,
    pub topic_id: String,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
            PathBuf::from(custom_path)
        } else {
            dirs_next::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("syntrix-admin")
        };
        Self::new_with_data_dir(data_dir).await
    }

    pub async fn new_with_data_dir(data_dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&data_dir).ok();

        let key_path = data_dir.join("keypair.bytes");
        let keypair = load_or_create_keypair(&key_path)?;

        let local_peer_id = keypair.public().to_peer_id();
        let listen_on: Vec<Multiaddr> = vec![
            "/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap(),
        ];

        let config = syntrix_network::NetworkConfig {
            keypair: keypair.clone(),
            listen_on,
            bootstrap_nodes: vec![],
            data_dir: data_dir.clone(),
        };

        let (p2p, event_rx) = P2PNode::new(config).await?;
        let registry = Arc::new(RwLock::new(NamespaceRegistry::new()));
        let db = crate::storage::open_limbo(&data_dir)?;
        crate::storage::run_migrations(&db)?;

        let heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>> =
            Arc::new(std::sync::RwLock::new(HashMap::new()));

        // Spawn background event processor
        let hb_ev = heartbeats.clone();
        let db_ev = db.clone();
        let p2p_ev = p2p.clone();
        let registry_ev = registry.clone();
        tokio::spawn(async move {
            process_event_loop(event_rx, hb_ev, db_ev, p2p_ev, registry_ev).await;
        });

        let mut orgs = HashMap::new();
        let mut devices: HashMap<String, HashMap<String, DeviceInfo>> = HashMap::new();
        let mut roles: HashMap<String, HashMap<String, RoleInfo>> = HashMap::new();

        // Load persisted devices & roles
        let devices_path = data_dir.join("devices.json");
        if devices_path.exists() {
            if let Ok(json_str) = std::fs::read_to_string(&devices_path) {
                if let Ok(persisted) = serde_json::from_str::<Vec<PersistedDevice>>(&json_str) {
                    for d in persisted {
                        devices
                            .entry(d.org.clone())
                            .or_default()
                            .insert(d.node_id.clone(), DeviceInfo {
                                node_id: d.node_id.clone(),
                                active: d.active,
                                role: d.role.clone(),
                                person: d.person.clone(),
                                name: d.name.clone(),
                                device_addr: d.device_addr.clone(),
                            });
                        if let Ok(mut reg) = registry.write() {
                            let node_id_bytes = hex::decode(&d.node_id).unwrap_or_default();
                            let mut id = [0u8; 32];
                            let len = node_id_bytes.len().min(32);
                            id[..len].copy_from_slice(&node_id_bytes[..len]);
                            reg.upsert_device(d.org.clone(), id, Device {
                                node_id: id,
                                active: d.active,
                                role: d.role.clone(),
                                person: d.person.clone(),
                                name: d.name.clone(),
                            });
                            let grants = default_role_grants(&d.role);
                            reg.upsert_role(d.org.clone(), d.role.clone(), RoleGrants {
                                can_open: grants.can_open,
                                can_write: grants.can_write,
                            });
                        }
                    }
                }
            }
        }

        let roles_path = data_dir.join("roles.json");
        if roles_path.exists() {
            if let Ok(json_str) = std::fs::read_to_string(&roles_path) {
                if let Ok(persisted) = serde_json::from_str::<Vec<PersistedRole>>(&json_str) {
                    for r in persisted {
                        roles
                            .entry(r.org.clone())
                            .or_default()
                            .insert(r.name.clone(), RoleInfo {
                                name: r.name.clone(),
                                can_open: r.can_open.clone(),
                                can_write: r.can_write.clone(),
                            });
                        if let Ok(mut reg) = registry.write() {
                            reg.upsert_role(r.org.clone(), r.name.clone(), RoleGrants {
                                can_open: r.can_open,
                                can_write: r.can_write,
                            });
                        }
                    }
                }
            }
        }

        // Load org config
        let orgs_config_path = data_dir.join("orgs.json");
        let node_id_bytes = peer_id_to_bytes(local_peer_id);
        let node_id_hex = hex::encode(node_id_bytes);
        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(configs) = serde_json::from_str::<Vec<OrgConfig>>(&orgs_json) {
                    for cfg in configs {
                        let topic_id_str = format!("syntrix-org-{}", cfg.name);

                        if let Ok(mut reg) = registry.write() {
                            reg.set_topic_id(cfg.name.clone(), topic_id_str.clone());
                        }

                        let org_name = cfg.name.clone();
                        orgs.insert(org_name.clone(), OrgState {
                            name: org_name.clone(),
                            topic_id: topic_id_str.clone(),
                        });
                        devices.entry(org_name.clone()).or_default();
                        roles.entry(org_name.clone()).or_default();

                        let _ = p2p.join_topic(&topic_id_str);

                        // Start admin heartbeat for this org
                        start_admin_heartbeat(
                            p2p.clone(),
                            org_name.clone(),
                            node_id_hex.clone(),
                            heartbeats.clone(),
                        ).await;
                    }
                }
            }
        }

        // Spawn background catchup from peers on restart
        let catchup_p2p = p2p.clone();
        let catchup_hb = heartbeats.clone();
        let catchup_db = db.clone();
        let catchup_orgs: Vec<String> = orgs.keys().cloned().collect();
        let catchup_self = node_id_hex.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            for org in &catchup_orgs {
                let hb_map = match catchup_hb.read() {
                    Ok(m) => m.clone(),
                    Err(_) => continue,
                };
                let org_hb = hb_map.get(org).cloned().unwrap_or_default();
                drop(hb_map);
                let now = chrono::Utc::now().timestamp_millis();
                for (peer_id, last_ts) in &org_hb {
                    if *peer_id == catchup_self { continue; }
                    if now - *last_ts > 120_000 { continue; }
                    if let Ok(peer_bytes) = hex::decode(peer_id) {
                        if peer_bytes.len() == 32 {
                            let mut arr = [0u8; 32];
                            arr.copy_from_slice(&peer_bytes);
                            if let Ok(peer) = libp2p::PeerId::from_bytes(&arr) {
                                let _ = catchup_from_peer(&catchup_p2p, peer, org, &catchup_db).await;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            keypair,
            p2p,
            event_rx: tokio::sync::Mutex::new(None),
            heartbeats,
            registry,
            orgs,
            devices,
            roles,
            data_dir,
            db,
        })
    }

    pub fn node_id(&self) -> NodeId { self.p2p.local_peer_id_bytes() }
    pub fn keypair(&self) -> &Keypair { &self.keypair }
    pub fn p2p(&self) -> &P2PNode { &self.p2p }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn data_dir(&self) -> &PathBuf { &self.data_dir }
    pub fn list_orgs(&self) -> Vec<String> { self.orgs.keys().cloned().collect() }

    pub fn get_heartbeats(&self, org: &str) -> HashMap<String, i64> {
        self.heartbeats
            .read()
            .unwrap()
            .get(org)
            .cloned()
            .unwrap_or_default()
    }

    pub fn add_org(&mut self, name: &str, topic_id: String) {
        self.orgs.insert(name.to_string(), OrgState { name: name.to_string(), topic_id });
        self.devices.entry(name.to_string()).or_default();
        self.roles.entry(name.to_string()).or_default();
    }

    pub fn save_org_config(&self, name: &str, topic_id: &str) -> anyhow::Result<()> {
        let orgs_config_path = self.data_dir.join("orgs.json");
        let mut configs = Vec::new();
        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(existing) = serde_json::from_str::<Vec<OrgConfig>>(&orgs_json) {
                    configs = existing;
                }
            }
        }
        configs.push(OrgConfig {
            name: name.to_string(),
            topic_id: topic_id.to_string(),
        });
        std::fs::write(&orgs_config_path, serde_json::to_vec(&configs)?)?;
        Ok(())
    }

    pub fn get_org(&self, name: &str) -> Option<&OrgState> { self.orgs.get(name) }

    pub async fn join_gossip_and_heartbeat(
        &self,
        org_name: &str,
        topic_id: String,
    ) -> anyhow::Result<()> {
        let _ = self.p2p.join_topic(&topic_id);
        let node_id_hex = hex::encode(self.node_id());
        start_admin_heartbeat(
            self.p2p.clone(),
            org_name.to_string(),
            node_id_hex,
            self.heartbeats.clone(),
        ).await;
        Ok(())
    }

    pub fn remember_device(&mut self, org: &str, node_id: &str, role: &str, person: &str, name: &str, active: bool, device_addr: &str) {
        self.devices.entry(org.into()).or_default().insert(node_id.into(), DeviceInfo {
            node_id: node_id.into(), active, role: role.into(), person: person.into(), name: name.into(),
            device_addr: device_addr.into(),
        });
        if let Ok(mut reg) = self.registry.write() {
            let node_id_bytes = hex::decode(node_id).unwrap_or_default();
            let mut id = [0u8; 32];
            let len = node_id_bytes.len().min(32);
            id[..len].copy_from_slice(&node_id_bytes[..len]);
            reg.upsert_device(org.into(), id, Device {
                node_id: id, active, role: role.into(), person: person.into(), name: name.into(),
            });
            let grants = default_role_grants(role);
            reg.upsert_role(org.into(), role.into(), RoleGrants {
                can_open: grants.can_open, can_write: grants.can_write,
            });
        }
        self.roles.entry(org.into()).or_default().entry(role.into()).or_insert_with(|| {
            let grants = default_role_grants(role);
            RoleInfo { name: role.into(), can_open: grants.can_open, can_write: grants.can_write }
        });

        self.save_devices();
        self.save_roles();
    }

    pub fn update_device(
        &mut self,
        org: &str,
        node_id: &str,
        active: bool,
        role: Option<String>,
        name: Option<String>,
        person: Option<String>,
    ) {
        if let Some(devs) = self.devices.get_mut(org) {
            if let Some(d) = devs.get_mut(node_id) {
                d.active = active;
                if let Some(r) = &role { d.role = r.clone(); }
                if let Some(n) = &name { d.name = n.clone(); }
                if let Some(p) = &person { d.person = p.clone(); }
            }
        }
        if let Ok(mut reg) = self.registry.write() {
            let node_id_bytes = hex::decode(node_id).unwrap_or_default();
            let mut id = [0u8; 32];
            let len = node_id_bytes.len().min(32);
            id[..len].copy_from_slice(&node_id_bytes[..len]);

            let mut current_role = String::new();
            let mut current_person = String::new();
            let mut current_name = String::new();

            if let Some(devs) = self.devices.get(org) {
                if let Some(d) = devs.get(node_id) {
                    current_role = d.role.clone();
                    current_person = d.person.clone();
                    current_name = d.name.clone();
                }
            }

            reg.upsert_device(org.into(), id, Device {
                node_id: id, active,
                role: role.unwrap_or(current_role),
                person: person.unwrap_or(current_person),
                name: name.unwrap_or(current_name),
            });
        }

        self.save_devices();
    }

    pub fn list_org_devices(&self, org: &str) -> Vec<DeviceInfo> {
        self.devices.get(org).map(|m| m.values().cloned().collect()).unwrap_or_default()
    }

    pub fn list_org_roles(&self, org: &str) -> Vec<RoleInfo> {
        self.roles.get(org).map(|m| m.values().cloned().collect()).unwrap_or_default()
    }

    pub fn set_role(&mut self, org: &str, name: &str, can_open: Vec<String>, can_write: Vec<String>) {
        if let Some(roles_map) = self.roles.get_mut(org) {
            roles_map.insert(name.into(), RoleInfo {
                name: name.into(),
                can_open: can_open.clone(),
                can_write: can_write.clone(),
            });
        }
        if let Ok(mut reg) = self.registry.write() {
            reg.upsert_role(org.into(), name.into(), RoleGrants {
                can_open,
                can_write,
            });
        }
        self.save_roles();
    }

    fn save_devices(&self) {
        let mut all: Vec<PersistedDevice> = Vec::new();
        for (org, devs) in &self.devices {
            for (_, d) in devs {
                all.push(PersistedDevice {
                    org: org.clone(),
                    node_id: d.node_id.clone(),
                    active: d.active,
                    role: d.role.clone(),
                    person: d.person.clone(),
                    name: d.name.clone(),
                    device_addr: d.device_addr.clone(),
                });
            }
        }
        let path = self.data_dir.join("devices.json");
        if let Ok(bytes) = serde_json::to_vec(&all) {
            let _ = std::fs::write(&path, bytes);
        }
    }

    fn save_roles(&self) {
        let mut all: Vec<PersistedRole> = Vec::new();
        for (org, roles_map) in &self.roles {
            for (_, r) in roles_map {
                all.push(PersistedRole {
                    org: org.clone(),
                    name: r.name.clone(),
                    can_open: r.can_open.clone(),
                    can_write: r.can_write.clone(),
                });
            }
        }
        let path = self.data_dir.join("roles.json");
        if let Ok(bytes) = serde_json::to_vec(&all) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}

pub fn default_role_grants(role: &str) -> RoleGrants {
    match role {
        "admin" => RoleGrants { can_open: vec!["*".into()], can_write: vec!["*".into()] },
        "sales" => RoleGrants { can_open: vec!["customers".into(),"products".into(),"invoices".into(),"orders".into()], can_write: vec!["customers".into(),"invoices".into(),"orders".into()] },
        "contabilidad" => RoleGrants { can_open: vec!["invoices".into(),"customers".into()], can_write: vec![] },
        _ => RoleGrants { can_open: vec![], can_write: vec![] },
    }
}

async fn start_admin_heartbeat(
    p2p: P2PNode,
    org_id: String,
    node_id_hex: String,
    heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>>,
) {
    let topic = format!("syntrix-org-{}", org_id);

    let broadcast: Arc<dyn Fn(&str) + Send + Sync> = {
        let p2p = p2p.clone();
        let topic = topic.clone();
        Arc::new(move |json: &str| {
            let p2p = p2p.clone();
            let t = topic.clone();
            let data = json.as_bytes().to_vec();
            tokio::spawn(async move {
                let _ = p2p.publish(&t, data);
            });
        })
    };

    tracing::info!(org = %org_id, op = "heartbeat", step = "start", node_id = %node_id_hex, "heartbeat started (every 15s)");

    let store: Arc<dyn Fn(i64, &str) + Send + Sync> = {
        let heartbeats = heartbeats;
        Arc::new(move |ts: i64, nid: &str| {
            if let Ok(mut map) = heartbeats.write() {
                map.entry(org_id.clone())
                    .or_default()
                    .insert(nid.to_string(), ts);
            }
        })
    };

    syntrix_core::heartbeat::start_heartbeat_with_resync(
        node_id_hex, broadcast, store,
    );
}

async fn process_event_loop(
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<Event>,
    heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>>,
    db: Arc<turso_core::Connection>,
    p2p: P2PNode,
    registry: Arc<RwLock<NamespaceRegistry>>,
) {
    use syntrix_network::Event;

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                match event {
                    Event::GossipsubMessage { source: _, topic, data } => {
                        let org_id = topic.strip_prefix("syntrix-org-").unwrap_or(&topic).to_string();
                        let content = match std::str::from_utf8(&data) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let val: serde_json::Value = match serde_json::from_str(content) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        if val.get("type").is_none() && val.get("kind").is_none() {
                            if let (Some(node_id), Some(ts)) = (
                                val.get("node_id").and_then(|v| v.as_str()),
                                val.get("ts").and_then(|v| v.as_i64()),
                            ) {
                                if let Ok(mut map) = heartbeats.write() {
                                    map.entry(org_id.clone())
                                        .or_default()
                                        .insert(node_id.to_string(), ts);
                                }
                            }
                            continue;
                        }

                        // CDC-native batches (Fase 3/4) carry entity data changes: applied to
                        // admin's own relational replica AND recorded as data-audit rows in
                        // event_log. The legacy JSON shape (still used for device.updated/
                        // role.updated broadcast by admin.rs itself) keeps going through
                        // write_event_to_limbo as a best-effort audit record.
                        if val.get("kind").and_then(|k| k.as_str()) == Some("cdc_batch") {
                            crate::gossip::apply_cdc_batch(&db, &org_id, &val, &registry);
                        } else {
                            write_event_to_limbo(&db, &org_id, &val);
                        }
                    }
                    Event::InviteReceived { peer: _, payload: _ } => {
                        // Admin doesn't receive invites
                    }
                    Event::CatchupRequestReceived { peer: _, org_id, since_hlc: _, response_id } => {
                        // Catch-up sends: (1) a full device/role roster (every peer needs the
                        // complete roster to validate `apply_cdc_events`'s author permission
                        // check, not just the devices it happened to be subscribed for when a
                        // `device.updated` was broadcast), and (2) a relational snapshot of
                        // the org's current entity/child rows as `CdcEvent`s, applied via the
                        // exact same `apply_cdc_events` used for live CDC sync (Fase 3, tarea
                        // 16). This replaces the old event-log replay, which broke once
                        // `event_log` became data-audit-only (Decision 5) — `row_image` no
                        // longer has the `{type, hlc, payload}` shape that replay expected.
                        let mut events: Vec<serde_json::Value> = Vec::new();
                        if let Ok(reg) = registry.read() {
                            for (role_name, grants) in reg.list_roles(&org_id) {
                                events.push(serde_json::json!({
                                    "type": "role.updated",
                                    "payload": {
                                        "name": role_name,
                                        "can_open": grants.can_open,
                                        "can_write": grants.can_write,
                                    },
                                }));
                            }
                            for device in reg.list_devices(&org_id) {
                                events.push(serde_json::json!({
                                    "type": "device.updated",
                                    "payload": {
                                        "node_id": hex::encode(device.node_id),
                                        "active": device.active,
                                        "role": device.role,
                                        "person": device.person,
                                        "name": device.name,
                                    },
                                }));
                            }
                        }
                        match syntrix_network::cdc::snapshot_org_rows(&db, &org_id) {
                            Ok(snapshot) if !snapshot.is_empty() => {
                                events.push(serde_json::json!({ "kind": "cdc_batch", "events": snapshot }));
                            }
                            Ok(_) => {}
                            Err(e) => {
                                tracing::warn!(target: "syntrix", org = %org_id, error = %e, "catchup: failed to snapshot org rows");
                            }
                        }
                        let _ = p2p.respond_catchup(response_id, events);
                    }
                    Event::PeerConnected(_) | Event::PeerDisconnected(_) => {}
                }
            }
            else => break,
        }
    }
}

fn write_event_to_limbo(db: &Arc<turso_core::Connection>, org_id: &str, val: &serde_json::Value) {
    crate::gossip::write_event_to_limbo(db, org_id, val);
}

fn peer_id_to_bytes(peer_id: libp2p::PeerId) -> [u8; 32] {
    let bytes = peer_id.to_bytes();
    let mut arr = [0u8; 32];
    let len = bytes.len().min(32);
    arr[..len].copy_from_slice(&bytes[..len]);
    arr
}

fn load_or_create_keypair(key_path: &std::path::Path) -> anyhow::Result<Keypair> {
    if key_path.exists() {
        let bytes = std::fs::read(key_path)?;
        if let Ok(kp) = Keypair::from_protobuf_encoding(&bytes) {
            return Ok(kp);
        }
        let kp = Keypair::generate_ed25519();
        std::fs::write(key_path, kp.to_protobuf_encoding().unwrap())?;
        Ok(kp)
    } else {
        let kp = Keypair::generate_ed25519();
        std::fs::write(key_path, kp.to_protobuf_encoding().unwrap())?;
        Ok(kp)
    }
}

pub async fn catchup_from_peer(
    p2p: &P2PNode,
    peer_id: libp2p::PeerId,
    org_id: &str,
    db: &Arc<turso_core::Connection>,
) -> anyhow::Result<()> {
    let events = p2p.request_catchup(peer_id, org_id.to_string(), 0).await?;
    let count = events.len();
    for event in &events {
        write_event_to_limbo(db, org_id, event);
    }
    tracing::info!(
        org = %org_id,
        peer = %peer_id,
        count = %count,
        "admin-catchup: received events from peer"
    );
    Ok(())
}
