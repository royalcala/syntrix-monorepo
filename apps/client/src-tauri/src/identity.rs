use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicU64;

use iroh::{Endpoint, SecretKey};
use iroh::endpoint::presets::N0;
use iroh::tls::CaRootsConfig;
use iroh_gossip::net::Gossip;
use syntrix_core::NodeId;
use syntrix_core::registry::NamespaceRegistry;
use crate::OrgInfo;
use serde::{Deserialize, Serialize};

pub use syntrix_core::parse_device_addr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientOrgConfig {
    pub org_id: String,
    pub name: String,
    pub role: String,
    pub topic_id: String,
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
        let orgs_config_path = data_dir.join("orgs.json");
        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(configs) = serde_json::from_str::<Vec<ClientOrgConfig>>(&orgs_json) {
                    for cfg in configs {
                        let topic_id_bytes = hex::decode(&cfg.topic_id).unwrap_or_default();
                        let mut topic_id = [0u8; 32];
                        let len = topic_id_bytes.len().min(32);
                        topic_id[..len].copy_from_slice(&topic_id_bytes[..len]);

                        if let Ok(mut reg) = registry.write() {
                            let iroh_topic_id = iroh_gossip::TopicId::from_bytes(topic_id);
                            reg.set_topic_id(cfg.org_id.clone(), iroh_topic_id);
                        }

                        orgs.insert(cfg.org_id.clone(), OrgState {
                            name: cfg.name,
                            role: cfg.role,
                            topic_id,
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

    pub fn add_org(&mut self, org_id: &str, name: &str, role: &str, topic_id: [u8; 32]) {
        self.orgs.insert(org_id.into(), OrgState {
            name: name.into(),
            role: role.into(),
            topic_id,
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
