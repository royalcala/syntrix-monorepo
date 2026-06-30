use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use iroh::{Endpoint, SecretKey};
use iroh::endpoint::presets::N0;
use iroh::tls::CaRootsConfig;
use syntrix_core::NodeId;
use syntrix_core::registry::{NamespaceRegistry, Device, RoleGrants};
use crate::{DeviceInfo, RoleInfo};
use serde::{Deserialize, Serialize};

pub use syntrix_core::parse_device_addr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrgConfig {
    pub name: String,
    pub topic_id: String,
}

// ----- persisted device / role shapes -----

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
    secret: SecretKey,
    _endpoint: Endpoint,
    _router: iroh::protocol::Router,
    #[allow(dead_code)]
    gossip: iroh_gossip::net::Gossip,
    pub gossip_bus: Arc<tokio::sync::RwLock<crate::gossip::AdminGossipBus>>,
    /// Synchronous-access heartbeat map (std RwLock). Updated by both the gossip
    /// receiver loop and the admin's own heartbeat store callback.  Reading this
    /// never requires a tokio lock, so it works from sync Tauri commands and tests.
    heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>>,
    registry: Arc<RwLock<NamespaceRegistry>>,
    orgs: HashMap<String, OrgState>,
    devices: HashMap<String, HashMap<String, DeviceInfo>>,
    roles: HashMap<String, HashMap<String, RoleInfo>>,
    data_dir: PathBuf,
    pub db: Arc<redb::Database>,
}

#[derive(Clone)]
pub struct OrgState {
    pub name: String,
    pub topic_id: [u8; 32],
}

const EVENT_LOG: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("event_log");

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
        let secret = if key_path.exists() {
            let bytes = std::fs::read(&key_path)?;
            if bytes.len() == 32 {
                let mut b = [0u8; 32];
                b.copy_from_slice(&bytes);
                SecretKey::from_bytes(&b)
            } else {
                let sk = SecretKey::generate();
                std::fs::write(&key_path, sk.to_bytes())?;
                sk
            }
        } else {
            let sk = SecretKey::generate();
            std::fs::write(&key_path, sk.to_bytes())?;
            sk
        };

        let ep = Endpoint::builder(N0)
            .secret_key(secret.clone())
            .ca_roots_config(CaRootsConfig::insecure_skip_verify())
            .bind_addr("0.0.0.0:0".parse::<std::net::SocketAddr>()?)?
            .bind()
            .await?;

        let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());
        let registry = Arc::new(RwLock::new(NamespaceRegistry::new()));
        let db_path = data_dir.join("admin.redb");
        let db = Arc::new(redb::Database::create(db_path)?);

        let gossip_bus = Arc::new(tokio::sync::RwLock::new(
            crate::gossip::AdminGossipBus::new(gossip.clone(), db.clone()),
        ));

        // Snapshot the std::sync heartbeats Arc so we can read it synchronously
        // without entering a tokio lock (needed by sync Tauri commands and tests).
        let heartbeats = {
            let guard = gossip_bus.read().await;
            guard.heartbeats_ref()
        };
        {
            let write_txn = db.begin_write()?;
            let _ = write_txn.open_table(EVENT_LOG)?;
            write_txn.commit()?;
        }

        let catchup_handler = crate::catchup::AdminCatchupProtocol::new(db.clone());
        let router = iroh::protocol::Router::builder(ep.clone())
            .accept(iroh_gossip::ALPN, gossip.clone())
            .accept(crate::catchup::CATCHUP_ALPN, catchup_handler)
            .spawn();

        let mut orgs = HashMap::new();
        let mut devices: HashMap<String, HashMap<String, DeviceInfo>> = HashMap::new();
        let mut roles: HashMap<String, HashMap<String, RoleInfo>> = HashMap::new();

        // ----- load persisted devices & roles BEFORE org loop (so lookup works) -----
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

        // ----- load org config -----
        let orgs_config_path = data_dir.join("orgs.json");
        let node_id_hex = hex::encode(*secret.public().as_bytes());
        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(configs) = serde_json::from_str::<Vec<OrgConfig>>(&orgs_json) {
                    for cfg in configs {
                        let topic_id_bytes = hex::decode(&cfg.topic_id).unwrap_or_default();
                        let mut topic_id = [0u8; 32];
                        let len = topic_id_bytes.len().min(32);
                        topic_id[..len].copy_from_slice(&topic_id_bytes[..len]);

                        if let Ok(mut reg) = registry.write() {
                            let iroh_topic_id = iroh_gossip::TopicId::from_bytes(topic_id);
                            reg.set_topic_id(cfg.name.clone(), iroh_topic_id);
                        }

                        let org_name = cfg.name.clone();
                        orgs.insert(org_name.clone(), OrgState {
                            name: org_name.clone(),
                            topic_id,
                        });
                        devices.entry(org_name.clone()).or_default();
                        roles.entry(org_name.clone()).or_default();

                        // Re-join gossip topic on restart
                        {
                            let mut bus = gossip_bus.write().await;
                            let iroh_topic_id = iroh_gossip::TopicId::from_bytes(topic_id);
                            let _ = bus.join_org(&org_name, iroh_topic_id).await;
                        }

                        // Start admin heartbeat for this org
                        start_admin_heartbeat(
                            gossip_bus.clone(),
                            org_name.clone(),
                            node_id_hex.clone(),
                        ).await;
                    }
                }
            }
        }

        Ok(Self {
            secret, _endpoint: ep, _router: router,
            gossip, gossip_bus, heartbeats, registry,
            orgs, devices, roles,
            data_dir, db,
        })
    }

    // ------------------------------------------------------------------
    // Accessors
    // ------------------------------------------------------------------

    pub fn node_id(&self) -> NodeId { *self.secret.public().as_bytes() }
    pub fn secret(&self) -> &SecretKey { &self.secret }
    pub fn endpoint(&self) -> &Endpoint { &self._endpoint }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn data_dir(&self) -> &PathBuf { &self.data_dir }
    pub fn list_orgs(&self) -> Vec<String> { self.orgs.keys().cloned().collect() }
    pub fn gossip_bus_ref(&self) -> &Arc<tokio::sync::RwLock<crate::gossip::AdminGossipBus>> { &self.gossip_bus }

    /// Read heartbeats for an org (used by `get_sync_info`).
    /// Uses the std::sync::RwLock copy — fully synchronous, works from tests.
    pub fn get_heartbeats(&self, org: &str) -> HashMap<String, i64> {
        self.heartbeats
            .read()
            .unwrap()
            .get(org)
            .cloned()
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // Org management
    // ------------------------------------------------------------------

    pub fn add_org(&mut self, name: &str, topic_id: [u8; 32]) {
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

    /// Join the gossip topic for an org and start the admin heartbeat.
    pub async fn join_gossip_and_heartbeat(
        &self,
        org_name: &str,
        topic_id: [u8; 32],
    ) -> anyhow::Result<()> {
        let iroh_topic_id = iroh_gossip::TopicId::from_bytes(topic_id);
        {
            let mut bus = self.gossip_bus.write().await;
            bus.join_org(org_name, iroh_topic_id).await?;
        }
        let node_id_hex = hex::encode(self.node_id());
        start_admin_heartbeat(
            self.gossip_bus.clone(),
            org_name.to_string(),
            node_id_hex,
        ).await;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Device management
    // ------------------------------------------------------------------

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

    // ------------------------------------------------------------------
    // Role management
    // ------------------------------------------------------------------

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

    // ------------------------------------------------------------------
    // Persistence helpers
    // ------------------------------------------------------------------

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

// ------------------------------------------------------------------
// Role defaults (unchanged)
// ------------------------------------------------------------------

pub fn default_role_grants(role: &str) -> RoleGrants {
    match role {
        "admin" => RoleGrants { can_open: vec!["*".into()], can_write: vec!["*".into()] },
        "sales" => RoleGrants { can_open: vec!["customers".into(),"products".into(),"invoices".into(),"orders".into()], can_write: vec!["customers".into(),"invoices".into(),"orders".into()] },
        "contabilidad" => RoleGrants { can_open: vec!["invoices".into(),"customers".into()], can_write: vec![] },
        _ => RoleGrants { can_open: vec![], can_write: vec![] },
    }
}

// ------------------------------------------------------------------
// Admin heartbeat (same pattern as client, but scoped to one org)
// ------------------------------------------------------------------

async fn start_admin_heartbeat(
    gossip_bus: Arc<tokio::sync::RwLock<crate::gossip::AdminGossipBus>>,
    org_id: String,
    node_id_hex: String,
) {
    // Snapshot the heartbeats Arc while we're in async context (no block_on needed).
    let hb = {
        let guard = gossip_bus.read().await;
        guard.heartbeats_ref()
    };

    let broadcast: Arc<dyn Fn(&str) + Send + Sync> = {
        let bus = gossip_bus.clone();
        let org = org_id.clone();
        Arc::new(move |json: &str| {
            let bus = bus.clone();
            let org = org.clone();
            let bytes = bytes::Bytes::copy_from_slice(json.as_bytes());
            tokio::spawn(async move {
                let mut guard = bus.write().await;
                let _ = guard.broadcast(&org, bytes).await;
            });
        })
    };

    tracing::info!(org = %org_id, op = "heartbeat", step = "start", node_id = %node_id_hex, "heartbeat started (every 15s)");

    let store: Arc<dyn Fn(i64, &str) + Send + Sync> = {
        let heartbeats = hb;
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
