use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicU64;

use iroh::{Endpoint, SecretKey};
use iroh::endpoint::presets::N0;
use iroh::tls::CaRootsConfig;
use iroh_gossip::net::Gossip;
use syntrix_core::NodeId;
use syntrix_core::registry::{NamespaceRegistry, Device, RoleGrants};
use crate::OrgInfo;
use serde::{Deserialize, Serialize};

pub use syntrix_core::parse_device_addr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientOrgConfig {
    pub org_id: String,
    pub name: String,
    pub role: String,
    pub topic_id: String,
    pub admin_addr: Option<String>,
    pub can_open: Vec<String>,
    pub can_write: Vec<String>,
}

pub struct AppState {
    secret: SecretKey,
    _endpoint: Endpoint,
    _router: iroh::protocol::Router,
    gossip: Gossip,
    hlc_counter: AtomicU64,
    registry: Arc<RwLock<NamespaceRegistry>>,
    orgs: HashMap<String, OrgState>,
    active_org: Option<String>,
    data_dir: PathBuf,
    pub indexer: Arc<crate::indexes::RelationalEngine>,
    pub gossip_bus: Arc<tokio::sync::RwLock<crate::gossip::GossipEventBus>>,
    pub invite_handler: crate::invite::InviteProtocolHandler,
    pub invite_rx: std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<crate::invite::InvitePayload>>>,
}

pub struct OrgState {
    pub name: String,
    pub role: String,
    pub topic_id: [u8; 32],
    pub admin_addr: Option<String>,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
            PathBuf::from(custom_path)
        } else {
            dirs_next::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("syntrix")
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
        ep.online().await;

        let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());
        let registry = Arc::new(RwLock::new(NamespaceRegistry::new()));

        let (invite_handler, invite_rx) = crate::invite::InviteProtocolHandler::new();
        let gossip_bus = Arc::new(tokio::sync::RwLock::new(crate::gossip::GossipEventBus::new(gossip.clone())));
        let indexer = Arc::new(crate::indexes::RelationalEngine::new(data_dir.clone())?);

        let router = iroh::protocol::Router::builder(ep.clone())
            .accept(iroh_gossip::ALPN, gossip.clone())
            .accept(crate::invite::INVITE_ALPN, invite_handler.clone())
            .spawn();

        let mut orgs = HashMap::new();
        let node_id = *secret.public().as_bytes();
        let node_id_hex = hex::encode(node_id);
        let orgs_config_path = data_dir.join("orgs.json");
        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(configs) = serde_json::from_str::<Vec<ClientOrgConfig>>(&orgs_json) {
                    for cfg in configs {
                        let topic_id_bytes = hex::decode(&cfg.topic_id).unwrap_or_default();
                        let mut topic_id_arr = [0u8; 32];
                        let len = topic_id_bytes.len().min(32);
                        topic_id_arr[..len].copy_from_slice(&topic_id_bytes[..len]);

                        if let Ok(mut reg) = registry.write() {
                            let iroh_topic_id = iroh_gossip::TopicId::from_bytes(topic_id_arr);
                            reg.set_topic_id(cfg.org_id.clone(), iroh_topic_id);
                            reg.upsert_device(cfg.org_id.clone(), node_id, Device {
                                node_id, active: true, role: cfg.role.clone(),
                                person: node_id_hex.clone(),
                                name: format!("Device {}", &node_id_hex[..8]),
                            });
                            reg.upsert_role(cfg.org_id.clone(), cfg.role.clone(),
                                RoleGrants { can_open: cfg.can_open.clone(), can_write: cfg.can_write.clone() });
                        }

                        // Write role to redb so frontend query_entity("roles") works
                        let role_json = serde_json::json!({
                            "name": cfg.role,
                            "can_open": cfg.can_open,
                            "can_write": cfg.can_write,
                        });
                        let _ = indexer.upsert_role_cfg(&cfg.org_id, &cfg.role, &role_json);

                        // Re-join gossip topic on startup
                        {
                            let mut bus = gossip_bus.write().await;
                            let iroh_topic_id = iroh_gossip::TopicId::from_bytes(topic_id_arr);
                            let bootstrap = cfg.admin_addr.as_ref()
                                .and_then(|a| syntrix_core::parse_device_addr(a))
                                .map(|ep| vec![ep.id])
                                .unwrap_or_default();
                            let _ = bus.join_org(
                                &cfg.org_id, iroh_topic_id, bootstrap,
                                indexer.clone(), node_id_hex.clone(),
                            ).await;
                        }

                        // Write self as member to redb
                        let self_member = serde_json::json!({
                            "node_id": node_id_hex,
                            "active": true,
                            "role": cfg.role,
                            "person": node_id_hex.clone(),
                            "name": format!("Device {}", &node_id_hex[..8]),
                        });
                        let _ = indexer.upsert_member(&cfg.org_id, &node_id_hex, &self_member);

                        // Start heartbeat per org
                        let hb_broadcast = {
                            let bus = gossip_bus.clone();
                            let org = cfg.org_id.clone();
                            std::sync::Arc::new(move |json: &str| {
                                let mut guard = bus.blocking_write();
                                let bytes = bytes::Bytes::copy_from_slice(json.as_bytes());
                                let handle = tokio::runtime::Handle::current();
                                let _ = handle.block_on(guard.broadcast(&org, bytes));
                            })
                        };
                        let hb_store = {
                            let idx = indexer.clone();
                            let org = cfg.org_id.clone();
                            std::sync::Arc::new(move |ts: i64, nid: &str| {
                                let hb = serde_json::json!({"ts": ts, "status": "online", "node_id": nid});
                                let _ = idx.upsert_heartbeat(&org, nid, &hb);
                            })
                        };
                        syntrix_core::heartbeat::start_heartbeat_with_resync(
                            node_id_hex.clone(), hb_broadcast, hb_store,
                        );

                        orgs.insert(cfg.org_id.clone(), OrgState {
                            name: cfg.name,
                            role: cfg.role,
                            topic_id: topic_id_arr,
                            admin_addr: cfg.admin_addr,
                        });
                    }
                }
            }
        }

        Ok(Self {
            secret, _endpoint: ep, _router: router,
            gossip, hlc_counter: AtomicU64::new(0), registry,
            orgs, active_org: None,
            data_dir, indexer, gossip_bus,
            invite_handler,
            invite_rx: std::sync::Mutex::new(Some(invite_rx)),
        })
    }

    pub fn node_id(&self) -> NodeId { *self.secret.public().as_bytes() }
    pub fn secret(&self) -> &SecretKey { &self.secret }
    pub fn counter(&self) -> &AtomicU64 { &self.hlc_counter }
    pub fn endpoint(&self) -> &Endpoint { &self._endpoint }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn data_dir(&self) -> &PathBuf { &self.data_dir }
    pub fn indexer(&self) -> Arc<crate::indexes::RelationalEngine> { self.indexer.clone() }

    pub fn list_orgs(&self) -> Vec<OrgInfo> {
        self.orgs.iter().map(|(id, o)| OrgInfo {
            id: id.clone(), name: o.name.clone(), role: o.role.clone(),
        }).collect()
    }

    pub fn set_active_org(&mut self, org_id: &str) -> Result<(), String> {
        if self.orgs.contains_key(org_id) {
            self.active_org = Some(org_id.to_string());
            Ok(())
        } else {
            Err(format!("org {} not found", org_id))
        }
    }

    pub fn active_org(&self) -> Result<&str, String> {
        self.active_org.as_deref().ok_or_else(|| "no active org".into())
    }

    pub fn add_org(&mut self, org_id: &str, name: &str, role: &str, topic_id: [u8; 32], admin_addr: Option<String>) {
        self.orgs.insert(org_id.into(), OrgState {
            name: name.into(),
            role: role.into(),
            topic_id,
            admin_addr,
        });
    }

    pub fn save_org_config(&self, cfg: ClientOrgConfig) -> anyhow::Result<()> {
        let orgs_config_path = self.data_dir.join("orgs.json");
        let mut configs = Vec::new();
        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(existing) = serde_json::from_str::<Vec<ClientOrgConfig>>(&orgs_json) {
                    configs = existing;
                }
            }
        }
        configs.retain(|c| c.org_id != cfg.org_id);
        configs.push(cfg);
        std::fs::write(&orgs_config_path, serde_json::to_vec(&configs)?)?;
        Ok(())
    }

    pub fn get_org(&self, org_id: &str) -> Option<&OrgState> {
        self.orgs.get(org_id)
    }
}


