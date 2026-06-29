use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use iroh::{Endpoint, SecretKey};
use iroh::endpoint::presets::N0;
use iroh::tls::CaRootsConfig;
use syntrix_core::NodeId;
use syntrix_core::registry::{NamespaceRegistry, Device, RoleGrants};
use iroh_docs::api::Doc;
use crate::{DeviceInfo, RoleInfo};
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrgConfig {
    pub name: String,
    pub control_id: String,
    pub namespace_ids: HashMap<String, String>,
}

// parse_device_addr is shared with syntrix-client — lives in syntrix-core.
pub use syntrix_core::parse_device_addr;



pub struct AppState {
    secret: SecretKey,
    _endpoint: Endpoint,
    _gossip: iroh_gossip::net::Gossip,
    _store: iroh_blobs::api::Store,
    _router: iroh::protocol::Router,
    docs_api: iroh_docs::api::DocsApi,
    author: iroh_docs::AuthorId,
    /// Shared namespace registry (updated on device changes, read by accept_cb).
    registry: Arc<RwLock<NamespaceRegistry>>,
    orgs: HashMap<String, OrgState>,
    devices: HashMap<String, HashMap<String, DeviceInfo>>,
    roles: HashMap<String, HashMap<String, RoleInfo>>,
    data_dir: PathBuf,
}

#[derive(Clone)]
pub struct OrgState {
    pub name: String,
    pub control_doc: Doc,
    pub entity_docs: HashMap<String, Doc>,
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

        let router = iroh::protocol::Router::builder(ep.clone())
            .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
            .accept(iroh_gossip::ALPN, gossip.clone())
            .accept(iroh_docs::ALPN, docs)
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

        let mut orgs = HashMap::new();
        let mut devices = HashMap::new();
        let mut roles = HashMap::new();

        let orgs_config_path = data_dir.join("orgs.json");
        if orgs_config_path.exists() {
            if let Ok(orgs_json) = std::fs::read_to_string(&orgs_config_path) {
                if let Ok(configs) = serde_json::from_str::<Vec<OrgConfig>>(&orgs_json) {
                    for cfg in configs {
                        let control_id = cfg.control_id.parse::<iroh_docs::NamespaceId>().ok();

                        let mut entity_docs: HashMap<String, Doc> = HashMap::new();
                        for (ns_name, ns_id_str) in &cfg.namespace_ids {
                            if let Ok(ns_id) = ns_id_str.parse::<iroh_docs::NamespaceId>() {
                                if let Some(doc) = api.open(ns_id).await.ok().flatten() {
                                    entity_docs.insert(ns_name.clone(), doc);
                                }
                            }
                        }

                        let ctrl_doc = match control_id {
                            Some(id) => api.open(id).await.ok().flatten(),
                            None => None,
                        };
                        if let Some(ctrl_doc) = ctrl_doc {
                            let name = cfg.name.clone();

                            // Register namespaces in Gossip accept callback registry
                            if let Ok(mut reg) = registry.write() {
                                reg.map_namespace_to_org(ctrl_doc.id(), name.clone());
                                for (_ns_name, doc) in &entity_docs {
                                    reg.map_namespace_to_org(doc.id(), name.clone());
                                }
                            }

                            // 1. Load members from control_doc
                            let mut dev_map = HashMap::new();
                            if let Ok(entries) = ctrl_doc.get_many(iroh_docs::store::Query::key_prefix("members/")).await {
                                let mut entries = Box::pin(entries);
                                while let Some(res) = entries.next().await {
                                    if let Ok(entry) = res {
                                        if let Ok(key) = std::str::from_utf8(entry.key()) {
                                            let node_id = key.strip_prefix("members/").unwrap_or(key).to_string();
                                            if let Ok(content_bytes) = store.blobs().get_bytes(entry.content_hash()).await {
                                                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&content_bytes) {
                                                    let active = val["active"].as_bool().unwrap_or(true);
                                                    let role = val["role"].as_str().unwrap_or("sales").to_string();
                                                    let person = val["person"].as_str().unwrap_or("").to_string();
                                                    let name_str = val["name"].as_str().unwrap_or("").to_string();
                                                    let device_addr = val["device_addr"].as_str().unwrap_or("").to_string();

                                                    dev_map.insert(node_id.clone(), DeviceInfo {
                                                        node_id: node_id.clone(),
                                                        active,
                                                        role: role.clone(),
                                                        person: person.clone(),
                                                        name: name_str.clone(),
                                                        device_addr: device_addr.clone(),
                                                    });

                                                    if let Ok(mut reg) = registry.write() {
                                                        if let Ok(node_id_bytes) = hex::decode(&node_id) {
                                                            let mut id = [0u8; 32];
                                                            let len = node_id_bytes.len().min(32);
                                                            id[..len].copy_from_slice(&node_id_bytes[..len]);
                                                            reg.upsert_device(name.clone(), id, Device {
                                                                node_id: id, active, role: role.clone(), person: person.clone(), name: name_str.clone(),
                                                            });
                                                        }
                                                    }

                                                    if active {
                                                        if let Some(endpoint_addr) = parse_device_addr(&device_addr) {
                                                            if endpoint_addr.id != secret.public() {
                                                                let peers_vec = vec![endpoint_addr];
                                                                let _ = ctrl_doc.start_sync(peers_vec.clone()).await;
                                                                for (_ns, doc) in &entity_docs {
                                                                    let _ = doc.start_sync(peers_vec.clone()).await;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            devices.insert(name.clone(), dev_map);

                            // 2. Load roles from control_doc
                            let mut role_map = HashMap::new();
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

                                                    role_map.insert(role_name.clone(), RoleInfo {
                                                        name: role_name.clone(),
                                                        can_open: can_open.clone(),
                                                        can_write: can_write.clone(),
                                                    });

                                                    if let Ok(mut reg) = registry.write() {
                                                        reg.upsert_role(name.clone(), role_name.clone(), syntrix_core::registry::RoleGrants {
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
                            roles.insert(name.clone(), role_map);

                            // Start heartbeat + periodic re-sync automatically
                            let node_id_hex = hex::encode(*secret.public().as_bytes());
                            let entity_docs_vec: Vec<iroh_docs::api::Doc> = entity_docs.values().cloned().collect();
                            crate::admin::start_heartbeat_with_resync(
                                ctrl_doc.clone(), entity_docs_vec,
                                author, node_id_hex, store.clone().into(), secret.clone(),
                                registry.clone(),
                            );

                            // Also update admin's own device_addr in control doc
                            let own_device_addr = crate::admin::build_device_addr_string(&ep);
                            let own_node_id = hex::encode(*secret.public().as_bytes());
                            let device_json = serde_json::json!({
                                "active": true,
                                "role": "admin",
                                "person": "admin",
                                "name": format!("Admin ({})", name),
                                "device_addr": own_device_addr,
                            });
                            let _ = ctrl_doc.set_bytes(
                                author, format!("members/{}", own_node_id).into_bytes(),
                                serde_json::to_vec(&device_json).unwrap_or_default(),
                            ).await;

                            orgs.insert(name.clone(), OrgState {
                                name,
                                control_doc: ctrl_doc,
                                entity_docs,
                            });
                        }
                    }
                }
            }
        }

        Ok(Self {
            secret, _endpoint: ep, _gossip: gossip, _store: store.clone().into(), _router: router,
            docs_api: api, author, registry,
            orgs,
            devices,
            roles,
            data_dir,
        })
    }

    pub fn node_id(&self) -> NodeId { *self.secret.public().as_bytes() }
    pub fn secret(&self) -> &SecretKey { &self.secret }
    pub fn api(&self) -> &iroh_docs::api::DocsApi { &self.docs_api }
    pub fn author(&self) -> iroh_docs::AuthorId { self.author }
    pub fn endpoint(&self) -> &Endpoint { &self._endpoint }
    pub fn store(&self) -> &iroh_blobs::api::Store { &self._store }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn data_dir(&self) -> &PathBuf { &self.data_dir }
    pub fn list_orgs(&self) -> Vec<String> { self.orgs.keys().cloned().collect() }

    pub fn add_org(&mut self, name: &str, control_doc: Doc, entity_docs: HashMap<String, Doc>) {
        self.orgs.insert(name.to_string(), OrgState { name: name.to_string(), control_doc, entity_docs });
        self.devices.entry(name.to_string()).or_default();
        self.roles.entry(name.to_string()).or_default();
    }

    pub fn save_org_config(&self, name: &str, ctrl: &str, namespace_ids: &HashMap<String, String>) -> anyhow::Result<()> {
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
            control_id: ctrl.to_string(),
            namespace_ids: namespace_ids.clone(),
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

    pub async fn sync_and_populate_org_members(&self, org_id: &str) -> anyhow::Result<()> {
        let org_state = match self.get_org(org_id) {
            Some(o) => o,
            None => return Ok(()),
        };
        let entity_docs: Vec<iroh_docs::api::Doc> = org_state.entity_docs.values().cloned().collect();
        sync_and_populate_org_members_impl(
            org_state.control_doc.clone(),
            entity_docs,
            self._store.clone(),
            self.registry.clone(),
            self.secret.clone(),
            org_id.to_string(),
        ).await
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

pub async fn sync_and_populate_org_members_impl(
    ctrl_doc: Doc,
    entity_docs: Vec<Doc>,
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
                            let role = val["role"].as_str().unwrap_or("sales").to_string();
                            let person = val["person"].as_str().unwrap_or("").to_string();
                            let name_str = val["name"].as_str().unwrap_or("").to_string();
                            let device_addr = val["device_addr"].as_str().unwrap_or("").to_string();

                            if let Ok(mut reg) = registry.write() {
                                if let Ok(node_id_bytes) = hex::decode(&node_id) {
                                    let mut id = [0u8; 32];
                                    let len = node_id_bytes.len().min(32);
                                    id[..len].copy_from_slice(&node_id_bytes[..len]);
                                    reg.upsert_device(org_id.clone(), id, Device {
                                        node_id: id, active, role: role.clone(), person: person.clone(), name: name_str.clone(),
                                    });
                                }
                            }

                            if active {
                                if let Some(endpoint_addr) = parse_device_addr(&device_addr) {
                                    if endpoint_addr.id != secret.public() {
                                        let peers_vec = vec![endpoint_addr];
                                        let _ = ctrl_doc.start_sync(peers_vec.clone()).await;
                                        for doc in &entity_docs {
                                            let _ = doc.start_sync(peers_vec.clone()).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Also load roles from control doc so the registry has up-to-date grants.
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

    Ok(())
}
