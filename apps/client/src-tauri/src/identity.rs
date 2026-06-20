use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicU64;

use iroh::{Endpoint, SecretKey};
use iroh::endpoint::presets::N0;
use iroh::tls::CaRootsConfig;
use iroh_syntrix_docs::NodeId;
use iroh_syntrix_docs::registry::NamespaceRegistry;
use iroh_docs::api::Doc;
use crate::{Invoice, Product, Customer, OrgInfo};
use crate::sync::SyncEntry;

pub struct AppState {
    secret: SecretKey,
    _endpoint: Endpoint,
    _gossip: iroh_gossip::net::Gossip,
    _store: iroh_blobs::store::mem::MemStore,
    _router: iroh::protocol::Router,
    _invite_router: iroh::protocol::Router,
    docs_api: iroh_docs::api::DocsApi,
    author: iroh_docs::AuthorId,
    hlc_counter: AtomicU64,
    registry: Arc<RwLock<NamespaceRegistry>>,
    orgs: HashMap<String, OrgState>,
    active_org: Option<String>,
    invoices: HashMap<String, Vec<Invoice>>,
    products: HashMap<String, Vec<Product>>,
    customers: HashMap<String, Vec<Customer>>,
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
        let secret = SecretKey::generate();
        let ep = Endpoint::builder(N0)
            .secret_key(secret.clone())
            .ca_roots_config(CaRootsConfig::insecure_skip_verify())
            .bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>()?)?
            .bind()
            .await?;
        ep.online().await;

        let store = iroh_blobs::store::mem::MemStore::new();
        let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());

        let registry = Arc::new(RwLock::new(NamespaceRegistry::new()));
        let accept_cb = iroh_syntrix_docs::accept::make_accept_cb(registry.clone());

        let docs = iroh_docs::protocol::Docs::memory()
            .accept_callback(accept_cb)
            .spawn(ep.clone(), (*store).clone(), gossip.clone())
            .await?;
        let api = docs.api().clone();

        let (invite_handler, invite_rx) = crate::invite::InviteProtocolHandler::new();

        let router = iroh::protocol::Router::builder(ep.clone())
            .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
            .accept(iroh_gossip::ALPN, gossip.clone())
            .accept(iroh_docs::ALPN, docs)
            .spawn();

        // Invite handler on a SEPARATE router (must not share with docs/gossip/blobs)
        let _invite_router = iroh::protocol::Router::builder(ep.clone())
            .accept(crate::invite::INVITE_ALPN, invite_handler.clone())
            .spawn();

        let author = api.author_create().await?;

        Ok(Self {
            secret, _endpoint: ep, _gossip: gossip, _store: store, _router: router, _invite_router,
            docs_api: api, author, hlc_counter: AtomicU64::new(0), registry,
            orgs: HashMap::new(), active_org: None,
            invoices: HashMap::new(), products: HashMap::new(), customers: HashMap::new(),
            events: HashMap::new(),
            invite_handler,
            invite_rx: std::sync::Mutex::new(Some(invite_rx)),
        })
    }

    pub fn node_id(&self) -> NodeId { *self.secret.public().as_bytes() }
    pub fn api(&self) -> &iroh_docs::api::DocsApi { &self.docs_api }
    pub fn author(&self) -> iroh_docs::AuthorId { self.author }
    pub fn counter(&self) -> &AtomicU64 { &self.hlc_counter }
    pub fn endpoint(&self) -> &Endpoint { &self._endpoint }
    pub fn registry(&self) -> &Arc<RwLock<NamespaceRegistry>> { &self.registry }

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
        self.invoices.entry(org_id.into()).or_default();
        self.products.entry(org_id.into()).or_default();
        self.customers.entry(org_id.into()).or_default();
    }

    pub fn get_org_docs(&self, org_id: &str) -> Option<&OrgState> {
        self.orgs.get(org_id)
    }

    pub fn add_invoice(&mut self, org_id: &str, invoice: Invoice) {
        self.invoices.entry(org_id.into()).or_default().push(invoice);
    }

    pub fn list_invoices(&self) -> Vec<Invoice> {
        let org_id = self.active_org.as_deref().unwrap_or("");
        self.invoices.get(org_id).cloned().unwrap_or_default()
    }

    pub fn list_products(&self) -> Vec<Product> {
        let org_id = self.active_org.as_deref().unwrap_or("");
        self.products.get(org_id).cloned().unwrap_or_default()
    }

    pub fn list_customers(&self) -> Vec<Customer> {
        let org_id = self.active_org.as_deref().unwrap_or("");
        self.customers.get(org_id).cloned().unwrap_or_default()
    }

    pub fn record_event(&mut self, org_id: &str, _seq_num: u64, entry: serde_json::Value) {
        self.events.entry(org_id.into()).or_default().push(crate::sync::SyncEntry { event_encoded: entry });
    }

    pub fn get_events_since(&self, org_id: &str, since_ts: u64) -> Vec<crate::sync::SyncEntry> {
        self.events.get(org_id).map(|entries| {
            entries.iter().filter(|e| {
                e.event_encoded.get("hlc").and_then(|h| h.get("ts")).and_then(|t| t.as_u64()).unwrap_or(0) > since_ts
            }).cloned().collect()
        }).unwrap_or_default()
    }
}
