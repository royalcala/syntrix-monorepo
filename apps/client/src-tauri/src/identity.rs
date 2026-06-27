use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicU64;

use iroh::{Endpoint, SecretKey};
use iroh::endpoint::presets::N0;
use iroh::tls::CaRootsConfig;
use syntrix_core::NodeId;
use syntrix_core::registry::{NamespaceRegistry, Device, RoleGrants};
use iroh_docs::api::Doc;
use crate::OrgInfo;
use crate::sync::SyncEntry;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientOrgConfig {
    pub org_id: String,
    pub name: String,
    pub role: String,
    pub control_id: String,
    pub catalogs_id: String,
    pub operational_id: String,
    pub payroll_id: String,
}

// parse_device_addr is shared with syntrix-admin — lives in syntrix-core.
pub use syntrix_core::parse_device_addr;


pub struct AppState {
    secret: SecretKey,
    _endpoint: Endpoint,
    _gossip: iroh_gossip::net::Gossip,
    _store: iroh_blobs::api::Store,
    _router: iroh::protocol::Router,
    docs_api: iroh_docs::api::DocsApi,
    author: iroh_docs::AuthorId,
    hlc_counter: AtomicU64,
    registry: Arc<RwLock<NamespaceRegistry>>,
    orgs: HashMap<String, OrgState>,
    active_org: Option<String>,
    data_dir: PathBuf,
    pub indexer: Arc<crate::indexes::RelationalEngine>,
    /// In-memory event buffer: org_id → (seqNum → entry), for pull.
    events: HashMap<String, Vec<SyncEntry>>,
    /// Invite protocol handler (receives org invitations from admin).
    pub invite_handler: crate::invite::InviteProtocolHandler,
    /// Channel receiver for real-time invite events (consumed by Tauri event emitter).
    pub invite_rx: std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<crate::invite::InvitePayload>>>,
}

pub struct OrgState {
    pub name: String,
    pub role: String,
    pub control_doc: Doc,
    pub catalogs_doc: Doc,
    pub operational_doc: Doc,
    pub payroll_doc: Doc,
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

        let blobs_dir = data_dir.join("blobs");
        std::fs::create_dir_all(&blobs_dir).ok();
        let store = iroh_blobs::store::fs::FsStore::load(&blobs_dir).await?;

        let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());

        let registry = Arc::new(RwLock::new(NamespaceRegistry::new()));
        let accept_cb = syntrix_core::make_accept_cb(registry.clone());

        let docs_dir = data_dir.join("docs");
        std::fs::create_dir_all(&docs_dir).ok();
        let docs = iroh_docs::protocol::Docs::persistent(docs_dir)
            .accept_callback(accept_cb)
            .spawn(ep.clone(), store.clone().into(), gossip.clone())
            .await?;
        let api = docs.api().clone();

        let (invite_handler, invite_rx) = crate::invite::InviteProtocolHandler::new();

        let router = iroh::protocol::Router::builder(ep.clone())
            .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
            .accept(iroh_gossip::ALPN, gossip.clone())
            .accept(iroh_docs::ALPN, docs)
            .accept(crate::invite::INVITE_ALPN, invite_handler.clone())
            .spawn();

        let author_path = data_dir.join("author.txt");
        let author = if author_path.exists() {
            let author_str = std::fs::read_to_string(&author_path)?;
            if let Ok(aut) = author_str.trim().parse::<iroh_docs::AuthorId>() {
                aut
            } else {
                let aut = api.author_create().await?;
                std::fs::write(&author_path, aut.to_string())?;
                aut
            }
        } else {
            let aut = api.author_create().await?;
            std::fs::write(&author_path, aut.to_string())?;
            aut
        };
        
        let indexer = Arc::new(crate::indexes::RelationalEngine::new(data_dir.clone())?);

        let mut orgs = HashMap::new();
        let orgs_config_path = data_dir.join("orgs.json");
        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(configs) = serde_json::from_str::<Vec<ClientOrgConfig>>(&orgs_json) {
                    for cfg in configs {
                        let control_id = cfg.control_id.parse::<iroh_docs::NamespaceId>().ok();
                        let catalogs_id = cfg.catalogs_id.parse::<iroh_docs::NamespaceId>().ok();
                        let operational_id = cfg.operational_id.parse::<iroh_docs::NamespaceId>().ok();
                        let payroll_id = cfg.payroll_id.parse::<iroh_docs::NamespaceId>().ok();

                        if let (Some(ctrl), Some(cat), Some(op), Some(pay)) = (control_id, catalogs_id, operational_id, payroll_id) {
                            let control_doc = api.open(ctrl).await.ok().flatten();
                            let catalogs_doc = api.open(cat).await.ok().flatten();
                            let operational_doc = api.open(op).await.ok().flatten();
                            let payroll_doc = api.open(pay).await.ok().flatten();

                            if let (Some(ctrl_doc), Some(cat_doc), Some(op_doc), Some(pay_doc)) = (control_doc, catalogs_doc, operational_doc, payroll_doc) {
                                let org_id = cfg.org_id.clone();
                                let name = cfg.name.clone();
                                let role = cfg.role.clone();
                                
                                // Register namespaces in Gossip accept callback registry
                                if let Ok(mut reg) = registry.write() {
                                    reg.map_namespace_to_org(ctrl, org_id.clone());
                                    reg.map_namespace_to_org(cat, org_id.clone());
                                    reg.map_namespace_to_org(op, org_id.clone());
                                    reg.map_namespace_to_org(pay, org_id.clone());
                                }

                                // 1. Load members from control_doc
                                if let Ok(entries) = ctrl_doc.get_many(iroh_docs::store::Query::key_prefix("members/")).await {
                                    let mut entries = Box::pin(entries);
                                    while let Some(res) = entries.next().await {
                                        if let Ok(entry) = res {
                                            if let Ok(key) = std::str::from_utf8(entry.key()) {
                                                let node_id = key.strip_prefix("members/").unwrap_or(key).to_string();
                                                if let Ok(content_bytes) = store.blobs().get_bytes(entry.content_hash()).await {
                                                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&content_bytes) {
                                                        let active = val["active"].as_bool().unwrap_or(true);
                                                        let r = val["role"].as_str().unwrap_or("sales").to_string();
                                                        let person = val["person"].as_str().unwrap_or("").to_string();
                                                        let name_str = val["name"].as_str().unwrap_or("").to_string();
                                                        let device_addr = val["device_addr"].as_str().unwrap_or("").to_string();

                                                        if let Ok(mut reg) = registry.write() {
                                                            if let Ok(node_id_bytes) = hex::decode(&node_id) {
                                                                let mut id = [0u8; 32];
                                                                let len = node_id_bytes.len().min(32);
                                                                id[..len].copy_from_slice(&node_id_bytes[..len]);
                                                                reg.upsert_device(org_id.clone(), id, Device {
                                                                    node_id: id, active, role: r.clone(), person: person.clone(), name: name_str.clone(),
                                                                });
                                                            }
                                                        }

                                                        if active {
                                                            if let Some(endpoint_addr) = parse_device_addr(&device_addr) {
                                                                if endpoint_addr.id != secret.public() {
                                                                    let peers_vec = vec![endpoint_addr];
                                                                    let _ = ctrl_doc.start_sync(peers_vec.clone()).await;
                                                                    let _ = cat_doc.start_sync(peers_vec.clone()).await;
                                                                    let _ = op_doc.start_sync(peers_vec.clone()).await;
                                                                    let _ = pay_doc.start_sync(peers_vec.clone()).await;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // 2. Load roles from control_doc
                                if let Ok(entries) = ctrl_doc.get_many(iroh_docs::store::Query::key_prefix("roles/")).await {
                                    let mut entries = Box::pin(entries);
                                    while let Some(res) = entries.next().await {
                                        if let Ok(entry) = res {
                                            if let Ok(key) = std::str::from_utf8(entry.key()) {
                                                let role_name = key.strip_prefix("roles/").unwrap_or(key).to_string();
                                                if let Ok(content_bytes) = store.blobs().get_bytes(entry.content_hash()).await {
                                                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&content_bytes) {
                                                        let can_open: Vec<String> = val["can_open"]
                                                            .as_array()
                                                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                                            .unwrap_or_default();
                                                        let can_write: Vec<String> = val["can_write"]
                                                            .as_array()
                                                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                                            .unwrap_or_default();

                                                        if let Ok(mut reg) = registry.write() {
                                                            reg.upsert_role(org_id.clone(), role_name.clone(), RoleGrants {
                                                                can_open: can_open.clone(),
                                                                can_write: can_write.clone(),
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                orgs.insert(org_id.clone(), OrgState {
                                    name,
                                    role,
                                    control_doc: ctrl_doc.clone(),
                                    catalogs_doc: cat_doc.clone(),
                                    operational_doc: op_doc.clone(),
                                    payroll_doc: pay_doc.clone(),
                                });

                                // Start heartbeat + periodic re-sync automatically
                                let node_id_hex = hex::encode(*secret.public().as_bytes());
                                crate::sync::start_heartbeat_with_resync(
                                    ctrl_doc.clone(), cat_doc.clone(), op_doc.clone(), pay_doc.clone(),
                                    author, node_id_hex, store.clone().into(), secret.clone(),
                                );

                                // Start remote entry ingestion via doc subscriptions
                                start_doc_subscriptions(
                                    ctrl_doc.clone(), cat_doc.clone(), op_doc.clone(), pay_doc.clone(),
                                    author, store.clone().into(), indexer.clone(), org_id.clone(),
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            secret, _endpoint: ep, _gossip: gossip, _store: store.clone().into(), _router: router,
            docs_api: api, author, hlc_counter: AtomicU64::new(0), registry,
            orgs, active_org: None,
            data_dir,
            indexer,
            events: HashMap::new(),
            invite_handler,
            invite_rx: std::sync::Mutex::new(Some(invite_rx)),
        })
    }

    pub fn node_id(&self) -> NodeId { *self.secret.public().as_bytes() }
    pub fn secret(&self) -> &SecretKey { &self.secret }
    pub fn api(&self) -> &iroh_docs::api::DocsApi { &self.docs_api }
    pub fn author(&self) -> iroh_docs::AuthorId { self.author }
    pub fn counter(&self) -> &AtomicU64 { &self.hlc_counter }
    pub fn endpoint(&self) -> &Endpoint { &self._endpoint }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn store(&self) -> &iroh_blobs::api::Store { &self._store }
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

    pub fn add_org_docs(&mut self, ns_name: &str, org_id: &str, name: &str, role: &str, doc: Doc) {
        if let Ok(mut reg) = self.registry.write() {
            reg.map_namespace_to_org(doc.id(), org_id.to_string());
        }
        let entry = self.orgs.entry(org_id.into()).or_insert_with(|| OrgState {
            name: name.into(), role: role.into(),
            control_doc: doc.clone(),
            catalogs_doc: doc.clone(),
            operational_doc: doc.clone(),
            payroll_doc: doc.clone(),
        });
        match ns_name {
            "control" => entry.control_doc = doc,
            "catalogs" => entry.catalogs_doc = doc,
            "operational" => entry.operational_doc = doc,
            "payroll" => entry.payroll_doc = doc,
            _ => {}
        }
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

        // Avoid duplicates
        configs.retain(|c| c.org_id != cfg.org_id);
        configs.push(cfg);

        std::fs::write(&orgs_config_path, serde_json::to_vec(&configs)?)?;
        Ok(())
    }

    pub fn get_org_docs(&self, org_id: &str) -> Option<&OrgState> {
        self.orgs.get(org_id)
    }

    pub fn record_event(&mut self, org_id: &str, _seq_num: u64, entry: serde_json::Value) {
        self.events.entry(org_id.into()).or_default().push(crate::sync::SyncEntry { event_encoded: entry });
    }

    pub async fn sync_and_populate_org_members(&self, org_id: &str) -> anyhow::Result<()> {
        let org_state = match self.get_org_docs(org_id) {
            Some(o) => o,
            None => return Ok(()),
        };
        sync_and_populate_org_members_impl(
            org_state.control_doc.clone(),
            org_state.catalogs_doc.clone(),
            org_state.operational_doc.clone(),
            org_state.payroll_doc.clone(),
            self._store.clone(),
            self.registry.clone(),
            self.secret.clone(),
            org_id.to_string(),
        ).await
    }

    pub fn get_events_since(&self, org_id: &str, since_ts: u64) -> Vec<crate::sync::SyncEntry> {
        self.events.get(org_id).map(|entries| {
            entries.iter().filter(|e| {
                e.event_encoded.get("hlc").and_then(|h| h.get("ts")).and_then(|t| t.as_u64()).unwrap_or(0) > since_ts
            }).cloned().collect()
        }).unwrap_or_default()
    }
}

pub async fn sync_and_populate_org_members_impl(
    ctrl_doc: Doc,
    cat_doc: Doc,
    op_doc: Doc,
    pay_doc: Doc,
    store: iroh_blobs::api::Store,
    registry: std::sync::Arc<std::sync::RwLock<NamespaceRegistry>>,
    secret: iroh::SecretKey,
    org_id: String,
) -> anyhow::Result<()> {
    if let Ok(entries) = ctrl_doc.get_many(iroh_docs::store::Query::key_prefix("members/")).await {
        let mut entries = Box::pin(entries);
        while let Some(res) = entries.next().await {
            if let Ok(entry) = res {
                if let Ok(key) = std::str::from_utf8(entry.key()) {
                    let node_id = key.strip_prefix("members/").unwrap_or(key).to_string();
                    if let Ok(content_bytes) = store.blobs().get_bytes(entry.content_hash()).await {
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&content_bytes) {
                            let active = val["active"].as_bool().unwrap_or(true);
                            let r = val["role"].as_str().unwrap_or("sales").to_string();
                            let person = val["person"].as_str().unwrap_or("").to_string();
                            let name_str = val["name"].as_str().unwrap_or("").to_string();
                            let device_addr = val["device_addr"].as_str().unwrap_or("").to_string();

                            if let Ok(mut reg) = registry.write() {
                                if let Ok(node_id_bytes) = hex::decode(&node_id) {
                                    let mut id = [0u8; 32];
                                    let len = node_id_bytes.len().min(32);
                                    id[..len].copy_from_slice(&node_id_bytes[..len]);
                                    reg.upsert_device(org_id.to_string(), id, Device {
                                        node_id: id, active, role: r.clone(), person: person.clone(), name: name_str.clone(),
                                    });
                                }
                            }

                            if active {
                                if let Some(endpoint_addr) = parse_device_addr(&device_addr) {
                                    if endpoint_addr.id != secret.public() {
                                        let peers_vec = vec![endpoint_addr];
                                        let _ = ctrl_doc.start_sync(peers_vec.clone()).await;
                                        let _ = cat_doc.start_sync(peers_vec.clone()).await;
                                        let _ = op_doc.start_sync(peers_vec.clone()).await;
                                        let _ = pay_doc.start_sync(peers_vec.clone()).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Start background document subscriptions for remote entry ingestion.
///
/// For each doc (control, catalogs, operational, payroll), spawns a task that:
/// 1. Scans existing `evt:` entries (catch-up for entries that arrived before subscribe)
/// 2. Subscribes to live events for ongoing remote entry indexing
///
/// Filters out entries authored by self to avoid redundant work.
pub fn start_doc_subscriptions(
    ctrl_doc: Doc,
    cat_doc: Doc,
    op_doc: Doc,
    pay_doc: Doc,
    author: iroh_docs::AuthorId,
    store: iroh_blobs::api::Store,
    indexer: Arc<crate::indexes::RelationalEngine>,
    org_id: String,
) {
    let docs = vec![
        ("control", ctrl_doc),
        ("catalogs", cat_doc),
        ("operational", op_doc),
        ("payroll", pay_doc),
    ];

    for (ns_name, doc) in docs {
        let store_clone = store.clone();
        let indexer_clone = indexer.clone();
        let author_clone = author;
        let org_id_clone = org_id.clone();
        let _ns = ns_name.to_string();

        tokio::spawn(async move {
            // Phase 1: initial scan of existing evt: entries
            if let Ok(stream) = doc.get_many(iroh_docs::store::Query::key_prefix("evt:")).await {
                let mut pinned = Box::pin(stream);
                while let Some(Ok(entry)) = pinned.next().await {
                    if entry.author() == author_clone { continue; }
                    process_and_index_entry(&entry, &store_clone, &*indexer_clone, &org_id_clone).await;
                }
            }

            // Phase 2: subscribe to live events
            if let Ok(mut live_stream) = doc.subscribe().await {
                use futures_util::StreamExt;
                while let Some(Ok(event)) = live_stream.next().await {
                    match event {
                        iroh_docs::engine::LiveEvent::InsertRemote { entry, .. } => {
                            if entry.author() == author_clone { continue; }
                            process_and_index_entry(&entry, &store_clone, &*indexer_clone, &org_id_clone).await;
                        }
                        iroh_docs::engine::LiveEvent::ContentReady { hash } => {
                            // Entries whose content was previously unavailable can be
                            // re-processed, but we don't maintain a stash yet.
                            let _ = hash;
                        }
                        _ => {}
                    }
                }
            }
        });
    }
}

/// Decode an iroh-docs entry, extract HLC + payload, and index it.
async fn process_and_index_entry(
    entry: &iroh_docs::Entry,
    store: &iroh_blobs::api::Store,
    indexer: &crate::indexes::RelationalEngine,
    org_id: &str,
) {
    let hash = entry.content_hash();
    let key_str = match std::str::from_utf8(entry.key()) {
        Ok(k) => k,
        Err(_) => return,
    };

    if !key_str.starts_with("evt:") { return; }

    let content_bytes = match store.blobs().get_bytes(hash).await {
        Ok(b) => b,
        Err(_) => return,
    };

    let content: serde_json::Value = match serde_json::from_slice(content_bytes.as_ref()) {
        Ok(v) => v,
        Err(_) => return,
    };

    let event_type = match content.get("type").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return,
    };

    let entity = crate::events::entity_from_event_type(event_type);
    let payload = match content.get("payload") {
        Some(p) => p.clone(),
        None => return,
    };

    let schema_version = content.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let doc_id = payload.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or(&key_str)
        .to_string();

    // Apply upcasters
    let upcasted = crate::events::upcast_payload(entity, payload, schema_version);

    // Extract HLC for LWW conflict resolution
    if let Some(hlc_val) = content.get("hlc") {
        let hlc = crate::indexes::HlcTimestamp {
            ts: hlc_val.get("ts").and_then(|v| v.as_u64()).unwrap_or(0),
            count: hlc_val.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            node: hlc_val.get("node").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };
        let _ = indexer.upsert_document_with_hlc(org_id, entity, &doc_id, &upcasted, &hlc);
    } else {
        let _ = indexer.upsert_document(org_id, entity, &doc_id, &upcasted);
    }
}

