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

pub struct AppState {
    secret: SecretKey,
    _endpoint: Endpoint,
    _router: iroh::protocol::Router,
    gossip: iroh_gossip::net::Gossip,
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
        let mut devices = HashMap::new();
        let mut roles = HashMap::new();

        let orgs_config_path = data_dir.join("orgs.json");
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
                        roles.entry(org_name).or_default();
                    }
                }
            }
        }

        Ok(Self {
            secret, _endpoint: ep, _router: router,
            gossip, registry,
            orgs, devices, roles,
            data_dir, db,
        })
    }

    pub fn node_id(&self) -> NodeId { *self.secret.public().as_bytes() }
    pub fn secret(&self) -> &SecretKey { &self.secret }
    pub fn endpoint(&self) -> &Endpoint { &self._endpoint }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn data_dir(&self) -> &PathBuf { &self.data_dir }
    pub fn list_orgs(&self) -> Vec<String> { self.orgs.keys().cloned().collect() }

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
    }

    pub fn list_org_devices(&self, org: &str) -> Vec<DeviceInfo> {
        self.devices.get(org).map(|m| m.values().cloned().collect()).unwrap_or_default()
    }

    pub fn list_org_roles(&self, org: &str) -> Vec<RoleInfo> {
        self.roles.get(org).map(|m| m.values().cloned().collect()).unwrap_or_default()
    }

    pub fn set_role(&mut self, org: &str, name: &str, can_open: Vec<String>, can_write: Vec<String>) {
        if let Some(roles) = self.roles.get_mut(org) {
            roles.insert(name.into(), RoleInfo {
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
    }
}

fn default_role_grants(role: &str) -> RoleGrants {
    match role {
        "admin" => RoleGrants { can_open: vec!["*".into()], can_write: vec!["*".into()] },
        "sales" => RoleGrants { can_open: vec!["customers".into(),"products".into(),"invoices".into(),"orders".into()], can_write: vec!["customers".into(),"invoices".into(),"orders".into()] },
        "contabilidad" => RoleGrants { can_open: vec!["invoices".into(),"customers".into()], can_write: vec![] },
        _ => RoleGrants { can_open: vec![], can_write: vec![] },
    }
}
