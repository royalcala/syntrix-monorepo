use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use iroh::{Endpoint, SecretKey};
use iroh::endpoint::presets::N0;
use iroh::tls::CaRootsConfig;
use iroh_syntrix_docs::NodeId;
use iroh_syntrix_docs::registry::NamespaceRegistry;
use iroh_docs::api::Doc;
use crate::{DeviceInfo, RoleInfo};

pub struct AppState {
    secret: SecretKey,
    _endpoint: Endpoint,
    _gossip: iroh_gossip::net::Gossip,
    _store: iroh_blobs::store::mem::MemStore,
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
        let secret = SecretKey::generate();
        let ep = Endpoint::builder(N0)
            .secret_key(secret.clone())
            .ca_roots_config(CaRootsConfig::insecure_skip_verify())
            .bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>()?)?
            .bind()
            .await?;

        let store = iroh_blobs::store::mem::MemStore::new();
        let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());

        let registry = Arc::new(RwLock::new(NamespaceRegistry::new()));
        let accept_cb = iroh_syntrix_docs::accept::make_accept_cb(registry.clone());

        let docs = iroh_docs::protocol::Docs::memory()
            .accept_callback(accept_cb)
            .spawn(ep.clone(), (*store).clone(), gossip.clone())
            .await?;
        let api = docs.api().clone();

        let router = iroh::protocol::Router::builder(ep.clone())
            .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
            .accept(iroh_gossip::ALPN, gossip.clone())
            .accept(iroh_docs::ALPN, docs)
            .spawn();

        let author = api.author_create().await?;

        Ok(Self {
            secret, _endpoint: ep, _gossip: gossip, _store: store, _router: router,
            docs_api: api, author, registry,
            orgs: HashMap::new(),
            devices: HashMap::new(),
            roles: HashMap::new(),
        })
    }

    pub fn node_id(&self) -> NodeId { *self.secret.public().as_bytes() }
    pub fn api(&self) -> &iroh_docs::api::DocsApi { &self.docs_api }
    pub fn author(&self) -> iroh_docs::AuthorId { self.author }
    pub fn endpoint(&self) -> &Endpoint { &self._endpoint }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }
    pub fn list_orgs(&self) -> Vec<String> { self.orgs.keys().cloned().collect() }

    pub fn add_org(&mut self, name: &str, control_doc: Doc, catalogs_doc: Doc, operational_doc: Doc, payroll_doc: Doc) {
        self.orgs.insert(name.to_string(), OrgState { name: name.to_string(), control_doc, catalogs_doc, operational_doc, payroll_doc });
        self.devices.entry(name.to_string()).or_default();
        self.roles.entry(name.to_string()).or_default();
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

    pub fn set_device_active(&mut self, org: &str, node_id: &str, active: bool) {
        if let Some(devs) = self.devices.get_mut(org) {
            if let Some(d) = devs.get_mut(node_id) { d.active = active; }
        }
        // Update registry too
        if let Ok(mut reg) = self.registry.write() {
            let node_id_bytes = hex::decode(node_id).unwrap_or_default();
            let mut id = [0u8; 32];
            let len = node_id_bytes.len().min(32);
            id[..len].copy_from_slice(&node_id_bytes[..len]);
            reg.upsert_device(org.into(), id, iroh_syntrix_docs::registry::Device {
                node_id: id, active,
                role: String::new(), person: String::new(), name: String::new(),
            });
        }
    }

    pub fn list_org_devices(&self, org: &str) -> Vec<DeviceInfo> {
        self.devices.get(org).map(|m| m.values().cloned().collect()).unwrap_or_default()
    }

    pub fn list_org_roles(&self, org: &str) -> Vec<RoleInfo> {
        self.roles.get(org).map(|m| m.values().cloned().collect()).unwrap_or_default()
    }
}

struct RoleGrants { can_open: Vec<String>, can_write: Vec<String> }

fn default_role_grants(role: &str) -> RoleGrants {
    match role {
        "admin" => RoleGrants { can_open: vec!["control".into(),"catalogs".into(),"operational".into(),"payroll".into()], can_write: vec!["catalogs".into(),"operational".into(),"payroll".into()] },
        "sales" => RoleGrants { can_open: vec!["control".into(),"catalogs".into(),"operational".into()], can_write: vec!["operational".into()] },
        "contabilidad" => RoleGrants { can_open: vec!["control".into(),"catalogs".into(),"operational".into(),"payroll".into()], can_write: vec![] },
        "hr" => RoleGrants { can_open: vec!["control".into(),"catalogs".into(),"payroll".into()], can_write: vec!["payroll".into()] },
        _ => RoleGrants { can_open: vec![], can_write: vec![] },
    }
}
