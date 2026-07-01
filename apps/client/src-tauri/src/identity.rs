use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

use libp2p::identity::Keypair;
use libp2p::Multiaddr;
use syntrix_core::NodeId;
use syntrix_core::registry::{Device, NamespaceRegistry, RoleGrants};
use syntrix_network::codecs::InvitePayload;
use syntrix_network::P2PNode;
use crate::invite::InviteHandler;
use crate::live::LiveManager;
use crate::OrgInfo;
use serde::{Deserialize, Serialize};

pub use syntrix_core::parse_device_addr;

use crate::indexes::{HlcTimestamp, SqlEngine};

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
    keypair: Keypair,
    p2p: P2PNode,
    hlc_counter: AtomicU64,
    registry: Arc<RwLock<NamespaceRegistry>>,
    orgs: HashMap<String, OrgState>,
    active_org: Option<String>,
    data_dir: PathBuf,
    pub indexer: Arc<crate::indexes::SqlEngine>,
    pub conn: Arc<turso_core::Connection>,
    pub live_manager: LiveManager,
    pub invite_handler: InviteHandler,
    /// Maps topic string → org_id for routing gossipsub messages
    topic_to_org: HashMap<String, String>,
}

pub struct OrgState {
    pub name: String,
    pub role: String,
    pub topic_id: String,
    pub admin_addr: Option<String>,
}

impl AppState {
    pub async fn new() -> anyhow::Result<(Self, Vec<InvitePayload>)> {
        let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
            PathBuf::from(custom_path)
        } else {
            dirs_next::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("syntrix")
        };
        Self::new_with_data_dir(data_dir).await
    }

    pub async fn new_with_data_dir(data_dir: PathBuf) -> anyhow::Result<(Self, Vec<InvitePayload>)> {
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
        let conn = crate::storage::open_limbo(&data_dir)?;
        crate::storage::run_migrations(&conn)?;
        let conn_arc = conn;
        let indexer = Arc::new(crate::indexes::SqlEngine::with_connection(conn_arc.clone()));
        let (invite_handler, _invite_rx) = InviteHandler::new();

        let mut topic_to_org: HashMap<String, String> = HashMap::new();
        let mut orgs = HashMap::new();
        let node_id_bytes: [u8; 32] = peer_id_to_bytes(local_peer_id);
        let node_id_hex = hex::encode(node_id_bytes);
        let orgs_config_path = data_dir.join("orgs.json");

        let idx_ev = indexer.clone();
        let p2p_ev = p2p.clone();
        let invite_handler_ev = invite_handler.clone();
        let topic_to_org_ev = topic_to_org.clone();

        // Spawn background event processor
        tokio::spawn(async move {
            process_event_loop(event_rx, idx_ev, p2p_ev, invite_handler_ev, topic_to_org_ev).await;
        });

        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(configs) = serde_json::from_str::<Vec<ClientOrgConfig>>(&orgs_json) {
                    for cfg in configs {
                        let topic_id_str = format!("syntrix-org-{}", cfg.org_id);
                        topic_to_org.insert(topic_id_str.clone(), cfg.org_id.clone());

                        if let Ok(mut reg) = registry.write() {
                            reg.set_topic_id(cfg.org_id.clone(), topic_id_str.clone());
                            reg.upsert_device(cfg.org_id.clone(), node_id_bytes, Device {
                                node_id: node_id_bytes,
                                active: true,
                                role: cfg.role.clone(),
                                person: node_id_hex.clone(),
                                name: format!("Device {}", &node_id_hex[..8]),
                            });
                            reg.upsert_role(cfg.org_id.clone(), cfg.role.clone(),
                                RoleGrants { can_open: cfg.can_open.clone(), can_write: cfg.can_write.clone() });
                        }

                        let role_json = serde_json::json!({
                            "name": cfg.role,
                            "can_open": cfg.can_open,
                            "can_write": cfg.can_write,
                        });
                        let _ = indexer.upsert_role_cfg(&cfg.org_id, &cfg.role, &role_json);

                        let _ = p2p.join_topic(&topic_id_str);

                        // Catch-up on restart
                        {
                            let idx = indexer.clone();
                            let node_hex = node_id_hex.clone();
                            let org = cfg.org_id.clone();
                            let p2p_catchup = p2p.clone();

                            let admin_ok = if let Some(ref addr_str) = cfg.admin_addr {
                                if let Some(peer_id) = syntrix_core::parse_device_addr(addr_str) {
                                    p2p_catchup.request_catchup(peer_id, org.clone(), 0).await.is_ok()
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            if !admin_ok {
                                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                                let hb = idx.get_heartbeats(&org).unwrap_or_default();
                                let now = chrono::Utc::now().timestamp_millis();
                                if let Some((peer_hex, _)) = hb.into_iter()
                                    .find(|(nid, ts)| nid != &node_hex && *ts > 0 && now - *ts < 60_000)
                                {
                                    if let Ok(peer_bytes) = hex::decode(&peer_hex) {
                                        if peer_bytes.len() == 32 {
                                            let mut arr = [0u8; 32];
                                            arr.copy_from_slice(&peer_bytes);
                                            if let Ok(peer_id) = libp2p::PeerId::from_bytes(&arr) {
                                                let _ = p2p_catchup.request_catchup(peer_id, org.clone(), 0).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let self_member = serde_json::json!({
                            "node_id": node_id_hex.clone(),
                            "active": true,
                            "role": cfg.role,
                            "person": node_id_hex.clone(),
                            "name": format!("Device {}", &node_id_hex[..8]),
                        });
                        let _ = indexer.upsert_member(&cfg.org_id, &node_id_hex, &self_member);

                        // Start heartbeat
                        let hb_broadcast = {
                            let p2p_hb = p2p.clone();
                            let topic = topic_id_str.clone();
                            Arc::new(move |json: &str| {
                                let p2p_hb = p2p_hb.clone();
                                let t = topic.clone();
                                let data = json.as_bytes().to_vec();
                                let _ = p2p_hb.publish(&t, data);
                            })
                        };
                        let hb_store = {
                            let idx = indexer.clone();
                            let org = cfg.org_id.clone();
                            Arc::new(move |ts: i64, nid: &str| {
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
                            topic_id: topic_id_str,
                            admin_addr: cfg.admin_addr,
                        });
                    }
                }
            }
        }

        Ok((
            Self {
                keypair,
                p2p,
                hlc_counter: AtomicU64::new(0),
                registry,
                orgs,
                active_org: None,
                data_dir,
                indexer,
                conn: conn_arc,
                live_manager: LiveManager::new(),
                invite_handler,
                topic_to_org,
            },
            vec![],
        ))
    }

    pub fn node_id(&self) -> NodeId { self.p2p.local_peer_id_bytes() }
    pub fn keypair(&self) -> &Keypair { &self.keypair }
    pub fn counter(&self) -> &AtomicU64 { &self.hlc_counter }
    pub fn p2p(&self) -> &P2PNode { &self.p2p }
    pub fn live_manager(&self) -> &LiveManager { &self.live_manager }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn data_dir(&self) -> &PathBuf { &self.data_dir }
    pub fn indexer(&self) -> Arc<crate::indexes::SqlEngine> { self.indexer.clone() }
    pub fn conn(&self) -> Arc<turso_core::Connection> { self.conn.clone() }

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

    pub fn add_org(&mut self, org_id: &str, name: &str, role: &str, topic_id: String, admin_addr: Option<String>) {
        self.topic_to_org.insert(topic_id.clone(), org_id.to_string());
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

async fn process_event_loop(
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<syntrix_network::Event>,
    indexer: Arc<crate::indexes::SqlEngine>,
    p2p: P2PNode,
    invite_handler: InviteHandler,
    topic_to_org: HashMap<String, String>,
) {
    use syntrix_network::Event;

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                match event {
                    Event::GossipsubMessage { source: _, topic, data } => {
                        let org_id = topic_to_org.get(&topic).cloned().unwrap_or_else(|| {
                            topic.strip_prefix("syntrix-org-").unwrap_or(&topic).to_string()
                        });
                        let content = match std::str::from_utf8(&data) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let val: serde_json::Value = match serde_json::from_str(content) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        process_gossip_event(&org_id, &val, &indexer);
                    }
                    Event::InviteReceived { peer: _, payload } => {
                        invite_handler.push(payload);
                    }
                    Event::CatchupRequestReceived { peer: _, org_id, since_hlc, response_id } => {
                        if let Ok(events) = indexer.query_events_since(&org_id, since_hlc, 10000) {
                            let event_data: Vec<serde_json::Value> = events.into_iter().map(|e| {
                                serde_json::json!({
                                    "type": e.event_type,
                                    "hlc": e.hlc,
                                    "schema_version": e.schema_version,
                                    "payload": e.payload,
                                })
                            }).collect();
                            let _ = p2p.respond_catchup(response_id, event_data);
                        }
                    }
                    Event::PeerConnected(_) | Event::PeerDisconnected(_) => {}
                }
            }
            else => break,
        }
    }
}

fn peer_id_to_bytes(peer_id: libp2p::PeerId) -> [u8; 32] {
    let bytes = peer_id.to_bytes();
    let mut arr = [0u8; 32];
    let len = bytes.len().min(32);
    arr[..len].copy_from_slice(&bytes[..len]);
    arr
}

fn load_or_create_keypair(key_path: &std::path::Path) -> anyhow::Result<Keypair> {
    use libp2p::identity::Keypair;
    if key_path.exists() {
        let bytes = std::fs::read(key_path)?;
        // Try protobuf format first
        if let Ok(kp) = Keypair::from_protobuf_encoding(&bytes) {
            return Ok(kp);
        }
        // Old 32-byte libp2p secret key — generate new, overwrite
        let kp = Keypair::generate_ed25519();
        std::fs::write(key_path, kp.to_protobuf_encoding().unwrap())?;
        Ok(kp)
    } else {
        let kp = Keypair::generate_ed25519();
        std::fs::write(key_path, kp.to_protobuf_encoding().unwrap())?;
        Ok(kp)
    }
}

fn process_gossip_event(
    org_id: &str,
    val: &serde_json::Value,
    indexer: &crate::indexes::SqlEngine,
) {
    if val.get("type").is_none() {
        if let Some(node_id) = val.get("node_id").and_then(|v| v.as_str()) {
            if let Some(_ts) = val.get("ts").and_then(|v| v.as_i64()) {
                let _ = indexer.upsert_heartbeat(org_id, node_id, val);
            }
        }
        return;
    }

    let event_type = match val.get("type").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return,
    };

    if event_type == "role.updated" {
        if let Some(payload) = val.get("payload") {
            if let Some(role_name) = payload.get("name").and_then(|v| v.as_str()) {
                let role_cfg = serde_json::json!({
                    "name": role_name,
                    "can_open": payload.get("can_open"),
                    "can_write": payload.get("can_write"),
                });
                let _ = indexer.upsert_role_cfg(org_id, role_name, &role_cfg);
                let _ = indexer.append_event(org_id, val);
            }
        }
        return;
    }

    if event_type == "device.updated" {
        if let Some(payload) = val.get("payload") {
            if let Some(node_id) = payload.get("node_id").and_then(|v| v.as_str()) {
                let _ = indexer.upsert_member(org_id, node_id, payload);
                let _ = indexer.append_event(org_id, val);
            }
        }
        return;
    }

    let entity = crate::events::entity_from_event_type(event_type);
    let payload = match val.get("payload") {
        Some(p) => p.clone(),
        None => return,
    };

    let doc_id = payload.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    if let Some(hlc_val) = val.get("hlc") {
        let hlc = crate::indexes::HlcTimestamp {
            ts: hlc_val.get("ts").and_then(|v| v.as_u64()).unwrap_or(0),
            count: hlc_val.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            node: hlc_val.get("node").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };
        let _ = indexer.upsert_document_with_hlc(org_id, entity, &doc_id, &payload, &hlc);
    } else {
        let _ = indexer.upsert_document(org_id, entity, &doc_id, &payload);
    }

    let _ = indexer.append_event(org_id, val);
}
