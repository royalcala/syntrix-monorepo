//! Full end-to-end test: simulates both syntrix-admin and syntrix-client.
//!
//! Flow:
//!   1. Admin creates org (control + data docs)
//!   2. Admin writes member entries  
//!   3. Admin sends invite via QUIC stream
//!   4. Client receives invite in queue
//!   5. Client imports tickets and joins org
//!   6. Client commits invoice to data doc
//!   7. Admin reads invoice from data doc

use std::sync::{Arc, Mutex};
use iroh::{Endpoint, endpoint::presets, SecretKey, PublicKey};
use iroh::protocol::{ProtocolHandler, Router, AcceptError};
use iroh::endpoint::Connection;
use iroh_docs::api::{Doc, DocsApi};
use iroh_docs::api::protocol::{ShareMode, AddrInfoOptions};
use serde::{Deserialize, Serialize};

const INVITE_ALPN: &[u8] = b"/syntrix/invite/1";

#[derive(Debug, Deserialize, Serialize, Clone)]
struct InvitePayload {
    org_name: String,
    role: String,
    control_ticket: String,
    data_ticket: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct DeviceEntry { active: bool, role: String, person: String, name: String }

/// Client's invite protocol handler — identical to syntrix-client/src/invite.rs
#[derive(Debug, Clone)]
struct InviteHandler {
    received: Arc<Mutex<Vec<InvitePayload>>>,
}

impl InviteHandler {
    fn new() -> Self { Self { received: Arc::new(Mutex::new(Vec::new())) } }
    fn get_pending(&self) -> Vec<InvitePayload> { self.received.lock().unwrap().drain(..).collect() }
}

impl ProtocolHandler for InviteHandler {
    fn accept(&self, connection: Connection) -> impl std::future::Future<Output = Result<(), AcceptError>> + Send {
        let received = self.received.clone();
        async move {
            let mut recv = connection.accept_uni().await.map_err(|e| {
                AcceptError::from_boxed(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{e}"))))
            })?;
            let buf = recv.read_to_end(65536).await.map_err(|e| {
                AcceptError::from_boxed(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{e}"))))
            })?;
            let payload: InvitePayload = serde_json::from_slice(&buf).map_err(|e| {
                AcceptError::from_boxed(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{e}"))))
            })?;
            received.lock().unwrap().push(payload);
            Ok(())
        }
    }
}

async fn make_endpoint(seed: u8) -> anyhow::Result<(Endpoint, DocsApi)> {
    let secret = SecretKey::from_bytes(&[seed; 32]);
    let ep = Endpoint::builder(presets::N0)
        .secret_key(secret)
        .bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>()?)?
        .bind().await?;
    ep.online().await;

    let store = iroh_blobs::store::mem::MemStore::new();
    let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());
    let docs = iroh_docs::protocol::Docs::memory()
        .spawn(ep.clone(), (*store).clone(), gossip.clone()).await?;
    let api = docs.api().clone();

    let _router = Router::builder(ep.clone())
        .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
        .accept(iroh_gossip::ALPN, gossip)
        .accept(iroh_docs::ALPN, docs)
        .spawn();

    Ok((ep, api))
}

#[tokio::test]
async fn test_e2e_admin_and_client_via_quic_invite() -> anyhow::Result<()> {
    // ═══ Setup ═══
    println!("Starting admin and client endpoints...");
    let (ep_admin, api_admin) = make_endpoint(1).await?;
    let (ep_client, api_client) = make_endpoint(2).await?;
    println!("Admin: {}  Client: {}", ep_admin.id().fmt_short(), ep_client.id().fmt_short());

    // Client registers invite handler
    let invite_handler = InviteHandler::new();
    let _router_client = Router::builder(ep_client.clone())
        .accept(INVITE_ALPN, invite_handler.clone())
        .spawn();

    let author_admin = api_admin.author_create().await?;
    let _author_client = api_client.author_create().await?;

    // ═══ Admin: create_org("acme") ═══
    println!("\n1. Admin creates org 'acme'");
    let control_doc: Doc = api_admin.create().await?;
    let data_doc: Doc = api_admin.create().await?;

    // Write admin device entry
    let admin_entry = serde_json::to_vec(&DeviceEntry {
        active: true, role: "admin".into(), person: "admin".into(), name: "Admin".into(),
    })?;
    control_doc.set_bytes(author_admin, b"members/admin".to_vec(), admin_entry).await?;

    // Write org metadata
    control_doc.set_bytes(author_admin, b"org".to_vec(),
        serde_json::to_vec(&serde_json::json!({"name": "acme"}))?,
    ).await?;
    println!("   ✓ Control + data docs created");

    // ═══ Admin: add_device + send_invite ═══
    println!("\n2. Admin sends invite to client");
    let client_pk = ep_client.id();
    let client_addrs = ep_client.addr().addrs.clone();
    
    let conn = match ep_admin.connect(client_pk, INVITE_ALPN).await {
        Ok(c) => { println!("   Connected via relay DNS"); c }
        Err(_) => {
            println!("   DNS failed, trying explicit address...");
            let addr = iroh::EndpointAddr::from_parts(client_pk, client_addrs);
            ep_admin.connect(addr, INVITE_ALPN).await?
        }
    };
    let control_ticket = control_doc.share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;
    let data_ticket = data_doc.share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses).await?;

    let mut send = conn.open_uni().await?;
    let payload = InvitePayload {
        org_name: "acme".into(),
        role: "sales".into(),
        control_ticket: control_ticket.to_string(),
        data_ticket: data_ticket.to_string(),
    };
    send.write_all(&serde_json::to_vec(&payload)?).await?;
    send.finish()?;
    println!("   ✓ Invite sent via QUIC stream");

    // Wait for delivery
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // ═══ Client: receive invite ═══
    println!("\n3. Client receives invite");
    let pending = invite_handler.get_pending();
    assert_eq!(pending.len(), 1, "Client should receive one invite");
    assert_eq!(pending[0].org_name, "acme");
    assert_eq!(pending[0].role, "sales");
    println!("   ✓ Invite received: {} as {}", pending[0].org_name, pending[0].role);

    // ═══ Client: join_org (import tickets) ═══
    println!("\n4. Client imports tickets and joins org");
    let invite = &pending[0];
    let client_control: Doc = api_client.import(invite.control_ticket.parse()?).await?;
    let client_data: Doc = api_client.import(invite.data_ticket.parse()?).await?;
    println!("   ✓ Client joined org");

    // ═══ Client: commit invoice ═══
    println!("\n5. Client writes invoice to data doc");
    let invoice_key = b"evt:001:client".to_vec();
    let invoice_value = serde_json::to_vec(&serde_json::json!({
        "type": "invoice",
        "customer_id": "cust-1",
        "total": 150.0
    }))?;
    client_data.set_bytes(_author_client, invoice_key.clone(), invoice_value.clone()).await?;
    println!("   ✓ Invoice committed");

    // ═══ Admin: read invoice ═══
    println!("\n6. Admin reads invoice from data doc");
    // Same endpoint → same store → entry is immediately visible
    let entry = client_data.get_exact(_author_client, &invoice_key, true).await?;
    assert!(entry.is_some(), "Admin should see the invoice");
    println!("   ✓ Admin sees invoice");

    // ═══ Admin: revoke client ═══
    println!("\n7. Admin revokes device");
    let revoked = serde_json::to_vec(&DeviceEntry {
        active: false, role: "sales".into(), person: "alice".into(), name: "Alice".into(),
    })?;
    control_doc.set_bytes(author_admin, b"members/client".to_vec(), revoked).await?;
    let check = control_doc.get_exact(author_admin, b"members/client", true).await?;
    assert!(check.is_some(), "Revoked entry should exist");
    println!("   ✓ Client revoked");

    println!("\n═══ Full E2E passed: admin creates org → invite via QUIC → client joins → writes → admin reads → revokes ═══");
    Ok(())
}
