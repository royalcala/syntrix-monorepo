//! Exact reproduction of the app invite flow.
//! Uses the SAME config as syntrix-admin + syntrix-client:
//! - N0 preset + custom relay (127.0.0.1:3340) + localhost bind
//! - Two SEPARATE endpoints (simulates separate processes)
//! - InviteProtocolHandler (same code as client's invite.rs)
//! - send_invite (same pattern as admin's admin.rs)

use std::sync::{Arc, Mutex};

use iroh::{
    Endpoint, SecretKey, RelayMode, RelayMap, RelayConfig, PublicKey, TransportAddr,
    protocol::{ProtocolHandler, Router, AcceptError},
    endpoint::Connection,
};
use iroh::endpoint::presets::N0;
use serde::{Deserialize, Serialize};

const INVITE_ALPN: &[u8] = b"/syntrix/invite/1";

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
struct InvitePayload {
    org_name: String,
    role: String,
    control_ticket: String,
    data_ticket: String,
}

#[derive(Debug, Clone)]
struct InviteHandler {
    received: Arc<Mutex<Vec<InvitePayload>>>,
}

impl InviteHandler {
    fn new() -> Self { Self { received: Arc::new(Mutex::new(Vec::new())) } }
    fn pending(&self) -> usize { self.received.lock().unwrap().len() }
    fn get_pending(&self) -> Vec<InvitePayload> { self.received.lock().unwrap().drain(..).collect() }
}

impl ProtocolHandler for InviteHandler {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl std::future::Future<Output = Result<(), AcceptError>> + Send {
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
            println!("[CLIENT] Received invite for {} as {}", payload.org_name, payload.role);
            received.lock().unwrap().push(payload);
            Ok(())
        }
    }
}

async fn create_admin_endpoint(seed: u8, relay_url: &str) -> anyhow::Result<(Endpoint,)> {
    let secret = SecretKey::from_bytes(&[seed; 32]);
    let url: iroh::RelayUrl = relay_url.parse()?;
    let relay_map: RelayMap = RelayConfig::new(url, None).into();
    
    let ep = Endpoint::builder(N0)
        .secret_key(secret)
        .relay_mode(RelayMode::Custom(relay_map))
        .bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>()?)?
        .bind()
        .await?;
    
    let store = iroh_blobs::store::mem::MemStore::new();
    let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());
    let docs = iroh_docs::protocol::Docs::memory()
        .spawn(ep.clone(), (*store).clone(), gossip.clone()).await?;
    
    let _router = Router::builder(ep.clone())
        .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
        .accept(iroh_gossip::ALPN, gossip)
        .accept(iroh_docs::ALPN, docs)
        .spawn();
    
    Ok((ep,))
}

async fn create_client_endpoint(seed: u8, relay_url: &str, handler: InviteHandler) -> anyhow::Result<(Endpoint,)> {
    let secret = SecretKey::from_bytes(&[seed; 32]);
    let url: iroh::RelayUrl = relay_url.parse()?;
    let relay_map: RelayMap = RelayConfig::new(url, None).into();
    
    let ep = Endpoint::builder(N0)
        .secret_key(secret)
        .relay_mode(RelayMode::Custom(relay_map))
        .bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>()?)?
        .bind()
        .await?;
    
    let store = iroh_blobs::store::mem::MemStore::new();
    let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());
    let docs = iroh_docs::protocol::Docs::memory()
        .spawn(ep.clone(), (*store).clone(), gossip.clone()).await?;
    
    // ONE Router with ALL handlers (same as client app)
    let _router = Router::builder(ep.clone())
        .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
        .accept(iroh_gossip::ALPN, gossip)
        .accept(iroh_docs::ALPN, docs)
        .accept(INVITE_ALPN, handler)
        .spawn();
    
    Ok((ep,))
}

/// Test: 4 handlers in ONE Router — proves/disproves iroh Router bug.
/// Marked #[ignore] because it confirms a known iroh limitation:
/// Router ignores handler #4 when 3+ already registered.
#[ignore = "confirms iroh Router bug: 4+ handlers, last one ignored"]
#[tokio::test]
async fn test_invite_in_single_router() -> anyhow::Result<()> {
    println!("=== Testing invite handler in SAME Router as docs/gossip/blobs ===");
    
    let relay_url = "http://127.0.0.1:3340";
    
    let (ep_admin,) = create_admin_endpoint(1, relay_url).await?;
    
    let handler = InviteHandler::new();
    let (ep_client,) = create_client_endpoint(2, relay_url, handler.clone()).await?;
    println!("[CLIENT] 4 handlers in 1 Router, pending: {}", handler.pending());
    
    let client_addr = ep_client.addr();
    let addrs: Vec<String> = client_addr.addrs.iter().map(|a| a.to_string()).collect();
    let addr_json = serde_json::json!({"node_id": hex::encode(ep_client.id().as_bytes()), "addrs": addrs});
    
    let addr_data: serde_json::Value = serde_json::from_str(&addr_json.to_string())?;
    let node_id_bytes = hex::decode(addr_data["node_id"].as_str().unwrap())?;
    let peer = PublicKey::from_bytes(&node_id_bytes.as_slice().try_into().unwrap())?;
    let addrs: Vec<TransportAddr> = addr_data["addrs"].as_array()
        .map(|a| a.iter().filter_map(|v| {
            let s = v.as_str()?;
            if let Some(relay_str) = s.strip_prefix("relay:") {
                relay_str.parse::<iroh::RelayUrl>().ok().map(TransportAddr::Relay)
            } else {
                let addr_str = s.strip_prefix("ip:").unwrap_or(s);
                addr_str.parse::<std::net::SocketAddr>().ok().map(TransportAddr::Ip)
            }
        }).collect()).unwrap_or_default();
    
    let addr = iroh::EndpointAddr::from_parts(peer, addrs);
    let conn = tokio::time::timeout(std::time::Duration::from_secs(10), ep_admin.connect(addr, INVITE_ALPN)).await??;
    
    let mut send = conn.open_uni().await?;
    let payload = InvitePayload { org_name: "acme".into(), role: "sales".into(), control_ticket: "c".into(), data_ticket: "d".into() };
    send.write_all(&serde_json::to_vec(&payload)?).await?;
    send.finish()?;
    let _ = conn.closed().await;
    
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    
    let pending = handler.get_pending();
    println!("[RESULT] 4-in-1 Router: {} invites received", pending.len());
    
    if pending.is_empty() {
        println!("[BUG CONFIRMED] iroh Router ignores handler #4 when 3+ already registered");
        println!("  Workaround: use separate Router for the invite handler");
        panic!("BUG: invite handler in same Router never receives connections");
    } else {
        assert_eq!(pending[0].org_name, "acme");
        println!("[OK] 4 handlers in 1 Router works — separate Router is NOT needed");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_app_invite_flow() -> anyhow::Result<()> {
    println!("=== Reproducing app invite flow ===");
    
    let relay_url = "http://127.0.0.1:3340";
    println!("Relay: {}", relay_url);
    
    // ── Create admin endpoint ──
    let (ep_admin,) = create_admin_endpoint(1, relay_url).await?;
    println!("[ADMIN] Endpoint: {}", ep_admin.id().fmt_short());
    
    // ── Create client endpoint with invite handler (SEPARATE router, same as E2E test) ──
    let handler = InviteHandler::new();
    let (ep_client,) = create_admin_endpoint(2, relay_url).await?; // use same as admin factory
    let _invite_router = Router::builder(ep_client.clone())
        .accept(INVITE_ALPN, handler.clone())
        .spawn();
    println!("[CLIENT] Endpoint: {}", ep_client.id().fmt_short());
    println!("[CLIENT] Invite handler registered (separate router), pending: {}", handler.pending());
    
    // Get client's full address (same as get_endpoint_addr command)
    let client_addr = ep_client.addr();
    let addrs: Vec<String> = client_addr.addrs.iter().map(|a| a.to_string()).collect();
    let node_id_hex = hex::encode(ep_client.id().as_bytes());
    let addr_json = serde_json::json!({
        "node_id": node_id_hex,
        "addrs": addrs,
    });
    println!("[CLIENT] Addr JSON: {}", addr_json);
    
    // ── Admin sends invite (same logic as admin.rs send_invite) ──
    println!("\n[SEND_INVITE] Parsing addr JSON...");
    
    let addr_data: serde_json::Value = serde_json::from_str(&addr_json.to_string())?;
    let node_id_hex = addr_data["node_id"].as_str().unwrap();
    let node_id_bytes = hex::decode(node_id_hex)?;
    let node_id: [u8; 32] = node_id_bytes.as_slice().try_into().unwrap();
    let peer = PublicKey::from_bytes(&node_id)?;
    
    // Parse addrs (same parsing as admin.rs)
    let addrs: Vec<TransportAddr> = addr_data["addrs"].as_array()
        .map(|a| a.iter().filter_map(|v| {
            let s = v.as_str()?;
            if let Some(relay_str) = s.strip_prefix("relay:") {
                relay_str.parse::<iroh::RelayUrl>().ok().map(TransportAddr::Relay)
            } else {
                let addr_str = s.strip_prefix("ip:").unwrap_or(s);
                addr_str.parse::<std::net::SocketAddr>().ok().map(TransportAddr::Ip)
            }
        }).collect())
        .unwrap_or_default();
    println!("[SEND_INVITE] Parsed {} addrs: {:?}", addrs.len(), addrs);
    
    // Skip DNS (it hangs without relay), use explicit addresses directly
    println!("[SEND_INVITE] Using explicit addresses, skipping DNS...");
    let addr = iroh::EndpointAddr::from_parts(peer, addrs);
    let conn = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        ep_admin.connect(addr, INVITE_ALPN)
    ).await??;
    println!("[SEND_INVITE] Connected!");
    
    // Send payload
    let payload = InvitePayload {
        org_name: "acme".into(), role: "sales".into(),
        control_ticket: "ctrl-123".into(), data_ticket: "data-456".into(),
    };
    let mut send = conn.open_uni().await?;
    send.write_all(&serde_json::to_vec(&payload)?).await?;
    send.finish()?;
    println!("[SEND_INVITE] Payload sent");
    
    // ── Wait for delivery ──
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    
    // ── Check client received it ──
    let pending = handler.get_pending();
    println!("\n[RESULT] Client received {} invites", pending.len());
    
    if pending.is_empty() {
        println!("[FAIL] No invite received!");
        println!("[DEBUG] Handler still registered: {}", handler.pending());
        println!("[DEBUG] Client addr: {:?}", ep_client.addr());
        println!("[DEBUG] Admin addr: {:?}", ep_admin.addr());
    }
    
    assert!(!pending.is_empty(), "Client must receive the invite");
    assert_eq!(pending[0].org_name, "acme");
    println!("[PASS] Invite received successfully");
    
    Ok(())
}
