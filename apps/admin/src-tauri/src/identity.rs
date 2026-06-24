use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use iroh::{Endpoint, SecretKey};
use iroh::endpoint::presets::N0;
use iroh::tls::CaRootsConfig;
use iroh_syntrix_docs::NodeId;
use iroh_syntrix_docs::registry::NamespaceRegistry;
use iroh_docs::api::Doc;
use crate::{DeviceInfo, RoleInfo};
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrgConfig {
    pub name: String,
    pub control_id: String,
    pub catalogs_id: String,
    pub operational_id: String,
    pub payroll_id: String,
}

fn parse_device_addr(addr_str: &str) -> Option<iroh::EndpointAddr> {
    if addr_str.is_empty() {
        return None;
    }
    if addr_str.contains(';') {
        let parts: Vec<&str> = addr_str.split(';').collect();
        let node_id_bytes = hex::decode(parts[0]).ok()?;
        let node_id: [u8; 32] = node_id_bytes.as_slice().try_into().ok()?;
        let peer = iroh::PublicKey::from_bytes(&node_id).ok()?;
        
        let addrs: Vec<iroh::TransportAddr> = parts[1..].iter().filter_map(|s| {
            if let Some(relay_str) = s.strip_prefix("relay:") {
                relay_str.parse::<iroh::RelayUrl>().ok().map(iroh::TransportAddr::Relay)
            } else {
                let s_addr = s.strip_prefix("ip:").unwrap_or(s);
                s_addr.parse::<std::net::SocketAddr>().ok().map(iroh::TransportAddr::Ip)
            }
        }).collect();
        Some(iroh::EndpointAddr::from_parts(peer, addrs))
    } else {
        let node_id_bytes = hex::decode(addr_str).ok()?;
        let node_id: [u8; 32] = node_id_bytes.as_slice().try_into().ok()?;
        let peer = iroh::PublicKey::from_bytes(&node_id).ok()?;
        Some(iroh::EndpointAddr::from_parts(peer, []))
    }
}

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
}

#[derive(Clone)]
pub struct OrgState {
    pub name: String,
    pub control_doc: Doc,
    pub catalogs_doc: Doc,
    pub operational_doc: Doc,
    pub payroll_doc: Doc,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
            std::path::PathBuf::from(custom_path)
        } else {
            dirs_next::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("syntrix-admin")
        };
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
        let accept_cb = iroh_syntrix_docs::accept::make_accept_cb(registry.clone());

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
                        let catalogs_id = cfg.catalogs_id.parse::<iroh_docs::NamespaceId>().ok();
                        let operational_id = cfg.operational_id.parse::<iroh_docs::NamespaceId>().ok();
                        let payroll_id = cfg.payroll_id.parse::<iroh_docs::NamespaceId>().ok();

                        if let (Some(ctrl), Some(cat), Some(op), Some(pay)) = (control_id, catalogs_id, operational_id, payroll_id) {
                            let control_doc = api.open(ctrl).await.ok().flatten();
                            let catalogs_doc = api.open(cat).await.ok().flatten();
                            let operational_doc = api.open(op).await.ok().flatten();
                            let payroll_doc = api.open(pay).await.ok().flatten();

                            if let (Some(ctrl_doc), Some(cat_doc), Some(op_doc), Some(pay_doc)) = (control_doc, catalogs_doc, operational_doc, payroll_doc) {
                                let name = cfg.name.clone();
                                
                                // Register namespaces in Gossip accept callback registry
                                if let Ok(mut reg) = registry.write() {
                                    reg.map_namespace_to_org(ctrl, name.clone());
                                    reg.map_namespace_to_org(cat, name.clone());
                                    reg.map_namespace_to_org(op, name.clone());
                                    reg.map_namespace_to_org(pay, name.clone());
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
                                                                reg.upsert_device(name.clone(), id, iroh_syntrix_docs::registry::Device {
                                                                    node_id: id, active, role: role.clone(), person: person.clone(), name: name_str.clone(),
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
                                                            reg.upsert_role(name.clone(), role_name.clone(), iroh_syntrix_docs::registry::RoleGrants {
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

                                // Start heartbeat sync automatically
                                let node_id_hex = hex::encode(*secret.public().as_bytes());
                                crate::admin::start_heartbeat(ctrl_doc.clone(), author, node_id_hex);

                                orgs.insert(name.clone(), OrgState {
                                    name,
                                    control_doc: ctrl_doc,
                                    catalogs_doc: cat_doc,
                                    operational_doc: op_doc,
                                    payroll_doc: pay_doc,
                                });
                            }
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
        })
    }

    pub fn node_id(&self) -> NodeId { *self.secret.public().as_bytes() }
    pub fn secret(&self) -> &SecretKey { &self.secret }
    pub fn api(&self) -> &iroh_docs::api::DocsApi { &self.docs_api }
    pub fn author(&self) -> iroh_docs::AuthorId { self.author }
    pub fn endpoint(&self) -> &Endpoint { &self._endpoint }
    pub fn store(&self) -> &iroh_blobs::api::Store { &self._store }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn list_orgs(&self) -> Vec<String> { self.orgs.keys().cloned().collect() }

    pub fn add_org(&mut self, name: &str, control_doc: Doc, catalogs_doc: Doc, operational_doc: Doc, payroll_doc: Doc) {
        self.orgs.insert(name.to_string(), OrgState { name: name.to_string(), control_doc, catalogs_doc, operational_doc, payroll_doc });
        self.devices.entry(name.to_string()).or_default();
        self.roles.entry(name.to_string()).or_default();
    }

    pub fn save_org_config(&self, name: &str, ctrl: &str, cat: &str, op: &str, pay: &str) -> anyhow::Result<()> {
        let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
            std::path::PathBuf::from(custom_path)
        } else {
            dirs_next::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("syntrix-admin")
        };
        let orgs_config_path = data_dir.join("orgs.json");
        
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
            catalogs_id: cat.to_string(),
            operational_id: op.to_string(),
            payroll_id: pay.to_string(),
        });

        std::fs::write(&orgs_config_path, serde_json::to_vec(&configs)?)?;
        Ok(())
    }

    pub fn get_org(&self, name: &str) -> Option<&OrgState> { self.orgs.get(name) }

    pub fn remember_device(&mut self, org: &str, node_id: &str, role: &str, person: &str, name: &str, active: bool, device_addr: &str) {
        // Update in-memory cache
        self.devices.entry(org.into()).or_default().insert(node_id.into(), DeviceInfo {
            node_id: node_id.into(), active, role: role.into(), person: person.into(), name: name.into(),
            device_addr: device_addr.into(),
        });
        // Update namespace registry (accept_cb reads this)
        if let Ok(mut reg) = self.registry.write() {
            let node_id_bytes = hex::decode(node_id).unwrap_or_default();
            let mut id = [0u8; 32];
            let len = node_id_bytes.len().min(32);
            id[..len].copy_from_slice(&node_id_bytes[..len]);
            reg.upsert_device(org.into(), id, iroh_syntrix_docs::registry::Device {
                node_id: id, active, role: role.into(), person: person.into(), name: name.into(),
            });
            // Auto-populate role grants (already uses new 4-namespace format)
            let grants = default_role_grants(role);
            reg.upsert_role(org.into(), role.into(), iroh_syntrix_docs::registry::RoleGrants {
                can_open: grants.can_open, can_write: grants.can_write,
            });
        }
        // Auto-add role to cache
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
        // Update registry too
        if let Ok(mut reg) = self.registry.write() {
            let node_id_bytes = hex::decode(node_id).unwrap_or_default();
            let mut id = [0u8; 32];
            let len = node_id_bytes.len().min(32);
            id[..len].copy_from_slice(&node_id_bytes[..len]);

            // Retain existing values if not provided
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

            reg.upsert_device(org.into(), id, iroh_syntrix_docs::registry::Device {
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
        // Update registry too
        if let Ok(mut reg) = self.registry.write() {
            reg.upsert_role(org.into(), name.into(), iroh_syntrix_docs::registry::RoleGrants {
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
}

struct RoleGrants { can_open: Vec<String>, can_write: Vec<String> }

fn default_role_grants(role: &str) -> RoleGrants {
    match role {
        "admin" => RoleGrants { can_open: vec!["customers".into(),"suppliers".into(),"products".into(),"invoices".into(),"orders".into()], can_write: vec!["customers".into(),"suppliers".into(),"products".into(),"invoices".into(),"orders".into()] },
        "sales" => RoleGrants { can_open: vec!["customers".into(),"products".into(),"invoices".into(),"orders".into()], can_write: vec!["customers".into(),"invoices".into(),"orders".into()] },
        "contabilidad" => RoleGrants { can_open: vec!["invoices".into(),"customers".into()], can_write: vec![] },
        _ => RoleGrants { can_open: vec![], can_write: vec![] },
    }
}

pub async fn sync_and_populate_org_members_impl(
    ctrl_doc: Doc,
    cat_doc: Doc,
    op_doc: Doc,
    pay_doc: Doc,
    store: iroh_blobs::api::Store,
    registry: std::sync::Arc<std::sync::RwLock<iroh_syntrix_docs::registry::NamespaceRegistry>>,
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
                                    reg.upsert_device(org_id.clone(), id, iroh_syntrix_docs::registry::Device {
                                        node_id: id, active, role: role.clone(), person: person.clone(), name: name_str.clone(),
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

