use std::collections::HashMap;
use std::num::NonZero;
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

fn run_to_completion(stmt: &mut turso_core::Statement) -> anyhow::Result<()> {
    loop {
        match stmt.step()? {
            turso_core::StepResult::Row => {}
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                stmt._io().step()?;
            }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                anyhow::bail!("database busy or interrupted");
            }
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrgConfig {
    pub name: String,
    pub topic_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingEnrollment {
    pub peer: libp2p::PeerId,
    pub peer_id_str: String,
    pub node_id: [u8; 32],
    pub org_name: String,
    pub response_id: u64,
}

pub struct AppState {
    keypair: Keypair,
    p2p: P2PNode,
    event_rx: tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Event>>>,
    heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>>,
    registry: Arc<RwLock<NamespaceRegistry>>,
    orgs: HashMap<String, OrgState>,
    data_dir: PathBuf,
    pub db: Arc<turso_core::Connection>,
    pending_enrollments: Arc<std::sync::RwLock<HashMap<u64, PendingEnrollment>>>,
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
            "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
            "/ip4/0.0.0.0/udp/0/quic-v1/p2p-circuit".parse().unwrap(),
        ];

        let bootstrap_nodes = syntrix_network::default_bootstrap_nodes();

        let config = syntrix_network::NetworkConfig {
            keypair: keypair.clone(),
            listen_on,
            bootstrap_nodes,
            data_dir: data_dir.clone(),
        };

        let (p2p, event_rx) = P2PNode::new(config).await?;
        let registry = Arc::new(RwLock::new(NamespaceRegistry::new()));
        let db = crate::storage::open_limbo(&data_dir)?;
        crate::storage::run_migrations(&db)?;

        let heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>> =
            Arc::new(std::sync::RwLock::new(HashMap::new()));
        let pending_enrollments: Arc<std::sync::RwLock<HashMap<u64, PendingEnrollment>>> =
            Arc::new(std::sync::RwLock::new(HashMap::new()));

        // Spawn background event processor
        let hb_ev = heartbeats.clone();
        let db_ev = db.clone();
        let p2p_ev = p2p.clone();
        let registry_ev = registry.clone();
        let enroll_ev = pending_enrollments.clone();
        tokio::spawn(async move {
            process_event_loop(event_rx, hb_ev, db_ev, p2p_ev, registry_ev, enroll_ev).await;
        });

        let node_id_bytes = peer_id_to_bytes(local_peer_id);
        let node_id_hex = hex::encode(node_id_bytes);

        // Load orgs from SQL (or migrate from legacy JSON)
        let orgs_config_path = data_dir.join("orgs.json");
        let mut orgs = HashMap::new();
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
                        let _ = p2p.join_topic(&topic_id_str);
                        start_admin_heartbeat(
                            p2p.clone(), org_name.clone(), node_id_hex.clone(), heartbeats.clone(),
                        ).await;
                        // Try to migrate legacy org into SQL
                        if let Ok(mut stmt) = db.prepare("INSERT OR IGNORE INTO admin_orgs (name, topic_id) VALUES (?1, ?2)") {
                            let _ = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_name.clone()));
                            let _ = stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(topic_id_str.clone()));
                            let _ = run_to_completion(&mut stmt);
                        }
                    }
                }
            }
        }

        // Load devices & roles from SQL into the registry
        if let Ok(mut stmt) = db.prepare("SELECT org_id, node_id, active, role, person, name FROM admin_devices") {
            loop {
                match stmt.step()? {
                    turso_core::StepResult::Row => {
                        if let Some(row) = stmt.row() {
                            let org: String = row.get(0)?;
                            let node_id_hex: String = row.get(1)?;
                            let active: i64 = row.get(2)?;
                            let role: String = row.get(3)?;
                            let person: String = row.get(4)?;
                            let name: String = row.get(5)?;
                            if let Ok(mut reg) = registry.write() {
                                let node_id_bytes = hex::decode(&node_id_hex).unwrap_or_default();
                                let mut id = [0u8; 32];
                                let len = node_id_bytes.len().min(32);
                                id[..len].copy_from_slice(&node_id_bytes[..len]);
                                reg.upsert_device(org, id, Device {
                                    node_id: id, active: active != 0, role, person, name,
                                });
                            }
                        }
                    }
                    turso_core::StepResult::Done => break,
                    turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                        stmt._io().step()?;
                    }
                    _ => {}
                }
            }
        }

        if let Ok(mut stmt) = db.prepare("SELECT org_id, role_name, can_open, can_write FROM admin_roles") {
            loop {
                match stmt.step()? {
                    turso_core::StepResult::Row => {
                        if let Some(row) = stmt.row() {
                            let org: String = row.get(0)?;
                            let role_name: String = row.get(1)?;
                            let can_open_str: String = row.get(2)?;
                            let can_write_str: String = row.get(3)?;
                            let can_open: Vec<String> = serde_json::from_str(&can_open_str).unwrap_or_default();
                            let can_write: Vec<String> = serde_json::from_str(&can_write_str).unwrap_or_default();
                            if let Ok(mut reg) = registry.write() {
                                reg.upsert_role(org, role_name, RoleGrants { can_open, can_write });
                            }
                        }
                    }
                    turso_core::StepResult::Done => break,
                    turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                        stmt._io().step()?;
                    }
                    _ => {}
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
            data_dir,
            db,
            pending_enrollments,
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

    pub fn list_pending_enrollments(&self) -> Vec<PendingEnrollment> {
        self.pending_enrollments.read().unwrap().values().cloned().collect()
    }

    pub fn pop_pending_enrollment(&self, response_id: u64) -> Option<PendingEnrollment> {
        self.pending_enrollments.write().unwrap().remove(&response_id)
    }

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

    pub fn remember_device(&self, org: &str, node_id: &str, role: &str, person: &str, name: &str, active: bool, device_addr: &str) {
        // Write to SQL
        if let Ok(mut stmt) = self.db.prepare(
            "INSERT OR REPLACE INTO admin_devices (org_id, node_id, active, role, person, name, device_addr, change_time) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch('now') * 1000)"
        ) {
            let _ = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org.to_string()));
            let _ = stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()));
            let _ = stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_i64(if active { 1 } else { 0 }));
            let _ = stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(role.to_string()));
            let _ = stmt.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_text(person.to_string()));
            let _ = stmt.bind_at(NonZero::new(6).unwrap(), turso_core::Value::from_text(name.to_string()));
            let _ = stmt.bind_at(NonZero::new(7).unwrap(), turso_core::Value::from_text(device_addr.to_string()));
            let _ = run_to_completion(&mut stmt);
        }

        // Register device in the in-memory registry
        if let Ok(mut reg) = self.registry.write() {
            let node_id_bytes = hex::decode(node_id).unwrap_or_default();
            let mut id = [0u8; 32];
            let len = node_id_bytes.len().min(32);
            id[..len].copy_from_slice(&node_id_bytes[..len]);
            reg.upsert_device(org.into(), id, Device {
                node_id: id, active, role: role.into(), person: person.into(), name: name.into(),
            });
            let existing_roles = reg.list_roles(&org.to_string());
            if !existing_roles.iter().any(|(name, _)| name == role) {
                let grants = default_role_grants(role);
                reg.upsert_role(org.into(), role.into(), RoleGrants {
                    can_open: grants.can_open, can_write: grants.can_write,
                });
            }
        }

        // Ensure default role exists in SQL
        if let Ok(mut stmt) = self.db.prepare(
            "INSERT OR IGNORE INTO admin_roles (org_id, role_name, can_open, can_write) VALUES (?1, ?2, ?3, ?4)"
        ) {
            let grants = default_role_grants(role);
            let can_open_json = serde_json::to_string(&grants.can_open).unwrap_or_else(|_| "[]".to_string());
            let can_write_json = serde_json::to_string(&grants.can_write).unwrap_or_else(|_| "[]".to_string());
            let _ = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org.to_string()));
            let _ = stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(role.to_string()));
            let _ = stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(can_open_json));
            let _ = stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(can_write_json));
            let _ = run_to_completion(&mut stmt);
        }
    }

    pub fn update_device(
        &mut self,
        org: &str,
        node_id: &str,
        active: bool,
        role: Option<String>,
        name: Option<String>,
        person: Option<String>,
        device_type: Option<String>,
    ) {
        if let Ok(mut stmt) = self.db.prepare(
            "UPDATE admin_devices SET active = ?3, role = COALESCE(?4, role), name = COALESCE(?5, name), person = COALESCE(?6, person), device_type = COALESCE(?7, device_type), change_time = unixepoch('now') * 1000 WHERE org_id = ?1 AND node_id = ?2"
        ) {
            let _ = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org.to_string()));
            let _ = stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()));
            let _ = stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_i64(if active { 1 } else { 0 }));
            let role_val = role.clone().unwrap_or_default();
            let _ = stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(role_val));
            let name_val = name.clone().unwrap_or_default();
            let _ = stmt.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_text(name_val));
            let person_val = person.clone().unwrap_or_default();
            let _ = stmt.bind_at(NonZero::new(6).unwrap(), turso_core::Value::from_text(person_val));
            let dt_val = device_type.unwrap_or_default();
            let _ = stmt.bind_at(NonZero::new(7).unwrap(), turso_core::Value::from_text(dt_val));
            let _ = run_to_completion(&mut stmt);
        }

        // Read current values from SQL for the registry update
        let current_role = role.unwrap_or_else(|| {
            if let Ok(mut stmt) = self.db.prepare("SELECT role FROM admin_devices WHERE org_id = ?1 AND node_id = ?2") {
                if let Ok(_) = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org.to_string())) {
                    let _ = stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()));
                    loop {
                        match stmt.step() {
                            Ok(turso_core::StepResult::Row) => {
                                if let Some(row) = stmt.row() {
                                    let r: String = row.get(0).unwrap_or_default();
                                    return r;
                                }
                            }
                            Ok(turso_core::StepResult::Done) => break,
                            Ok(turso_core::StepResult::IO | turso_core::StepResult::Yield) => { let _ = stmt._io().step(); }
                            _ => break,
                        }
                    }
                }
            }
            String::new()
        });

        if let Ok(mut reg) = self.registry.write() {
            let node_id_bytes = hex::decode(node_id).unwrap_or_default();
            let mut id = [0u8; 32];
            let len = node_id_bytes.len().min(32);
            id[..len].copy_from_slice(&node_id_bytes[..len]);
            reg.upsert_device(org.into(), id, Device {
                node_id: id, active,
                role: current_role,
                person: person.clone().unwrap_or_default(),
                name: name.clone().unwrap_or_default(),
            });
        }
    }

    pub fn list_org_devices(&self, org: &str) -> Vec<DeviceInfo> {
        let mut devices = Vec::new();
        if let Ok(mut stmt) = self.db.prepare("SELECT node_id, active, role, person, name, device_addr, device_type FROM admin_devices WHERE org_id = ?1") {
            if let Ok(_) = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org.to_string())) {
                loop {
                    match stmt.step() {
                        Ok(turso_core::StepResult::Row) => {
                            if let Some(row) = stmt.row() {
                                devices.push(DeviceInfo {
                                    node_id: row.get::<String>(0).unwrap_or_default(),
                                    active: row.get::<i64>(1).unwrap_or(1) != 0,
                                    role: row.get::<String>(2).unwrap_or_default(),
                                    person: row.get::<String>(3).unwrap_or_default(),
                                    name: row.get::<String>(4).unwrap_or_default(),
                                    device_addr: row.get::<String>(5).unwrap_or_default(),
                                    device_type: row.get::<String>(6).unwrap_or_default(),
                                });
                            }
                        }
                        Ok(turso_core::StepResult::Done) => break,
                        Ok(turso_core::StepResult::IO | turso_core::StepResult::Yield) => {
                            let _ = stmt._io().step();
                        }
                        _ => break,
                    }
                }
            }
        }
        devices
    }

    pub fn list_org_roles(&self, org: &str) -> Vec<RoleInfo> {
        let mut roles = Vec::new();
        if let Ok(mut stmt) = self.db.prepare("SELECT role_name, can_open, can_write FROM admin_roles WHERE org_id = ?1") {
            if let Ok(_) = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org.to_string())) {
                loop {
                    match stmt.step() {
                        Ok(turso_core::StepResult::Row) => {
                            if let Some(row) = stmt.row() {
                                let can_open_str: String = row.get::<String>(1).unwrap_or_else(|_| "[]".to_string());
                                let can_write_str: String = row.get::<String>(2).unwrap_or_else(|_| "[]".to_string());
                                roles.push(RoleInfo {
                                    name: row.get::<String>(0).unwrap_or_default(),
                                    can_open: serde_json::from_str(&can_open_str).unwrap_or_default(),
                                    can_write: serde_json::from_str(&can_write_str).unwrap_or_default(),
                                });
                            }
                        }
                        Ok(turso_core::StepResult::Done) => break,
                        Ok(turso_core::StepResult::IO | turso_core::StepResult::Yield) => {
                            let _ = stmt._io().step();
                        }
                        _ => break,
                    }
                }
            }
        }
        roles
    }

    pub fn set_role(&mut self, org: &str, name: &str, can_open: Vec<String>, can_write: Vec<String>) {
        let can_open_json = serde_json::to_string(&can_open).unwrap_or_else(|_| "[]".to_string());
        let can_write_json = serde_json::to_string(&can_write).unwrap_or_else(|_| "[]".to_string());
        if let Ok(mut stmt) = self.db.prepare(
            "INSERT OR REPLACE INTO admin_roles (org_id, role_name, can_open, can_write, change_time) VALUES (?1, ?2, ?3, ?4, unixepoch('now') * 1000)"
        ) {
            let _ = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org.to_string()));
            let _ = stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(name.to_string()));
            let _ = stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(can_open_json));
            let _ = stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(can_write_json));
            let _ = run_to_completion(&mut stmt);
        }
        if let Ok(mut reg) = self.registry.write() {
            reg.upsert_role(org.into(), name.into(), RoleGrants { can_open, can_write });
        }
    }
}

pub fn default_role_grants(role: &str) -> RoleGrants {
    match role {
        "admin" => RoleGrants { can_open: vec!["*".into()], can_write: vec!["*".into()] },
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
    pending_enrollments: Arc<std::sync::RwLock<HashMap<u64, PendingEnrollment>>>,
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
                    Event::EnrollRequestReceived { peer, node_id, peer_id_str, org_name, response_id } => {
                        tracing::info!(target: "syntrix", org = %org_name, peer = %peer, "enroll request received");
                        let enrollment = PendingEnrollment {
                            peer,
                            peer_id_str,
                            node_id,
                            org_name,
                            response_id,
                        };
                        if let Ok(mut map) = pending_enrollments.write() {
                            map.insert(response_id, enrollment);
                        }
                    }
                    Event::PeerConnected(pid) => {
                        tracing::info!(target: "syntrix", peer = %pid, "peer connected");
                    }
                    Event::PeerDisconnected(pid) => {
                        tracing::info!(target: "syntrix", peer = %pid, "peer disconnected, scheduling reconnect");
                        let p2p = p2p.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            p2p.ensure_connected(pid, vec![]);
                        });
                    }
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
