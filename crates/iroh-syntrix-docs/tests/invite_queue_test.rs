//! Reproduction test: exact same flow as admin → client invite
//! Creates two endpoints, registers invite handler on B, sends from A, checks B's queue.

use std::sync::{Arc, Mutex};
use iroh::{Endpoint, endpoint::presets, SecretKey, PublicKey};
use iroh::protocol::{ProtocolHandler, Router, AcceptError};
use iroh::endpoint::Connection;
use serde::{Deserialize, Serialize};

const INVITE_ALPN: &[u8] = b"/syntrix/invite/1";

#[derive(Debug, Deserialize, Serialize, Clone)]
struct InvitePayload {
    org_name: String,
    role: String,
    control_ticket: String,
    data_ticket: String,
}

#[derive(Debug, Clone)]
struct Receiver {
    received: Arc<Mutex<Vec<InvitePayload>>>,
}

impl Receiver {
    fn new() -> Self { Self { received: Arc::new(Mutex::new(Vec::new())) } }
    fn get_pending(&self) -> Vec<InvitePayload> {
        self.received.lock().unwrap().drain(..).collect()
    }
}

impl ProtocolHandler for Receiver {
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
            println!("Receiver: got invite for {} as {}", payload.org_name, payload.role);
            received.lock().unwrap().push(payload);
            Ok(())
        }
    }
}

#[tokio::test]
async fn test_invite_handler_and_queue() -> anyhow::Result<()> {
    // ── Setup client (B) with invite handler ──
    let secret_b = SecretKey::from_bytes(&[2u8; 32]);
    let pk_b: PublicKey = secret_b.public();
    let ep_b = Endpoint::builder(presets::N0)
        .secret_key(secret_b)
        .bind().await?;
    ep_b.online().await;

    let receiver = Receiver::new();
    let receiver_for_router = receiver.clone();
    let _router_b = Router::builder(ep_b.clone())
        .accept(INVITE_ALPN, receiver_for_router)
        .spawn();
    println!("B online at {}", ep_b.id().fmt_short());

    // ── Setup admin (A) ──
    let secret_a = SecretKey::from_bytes(&[1u8; 32]);
    let ep_a = Endpoint::builder(presets::N0)
        .secret_key(secret_a).bind().await?;
    ep_a.online().await;
    println!("A online at {}", ep_a.id().fmt_short());

    // ── A sends invite to B ──
    let conn = match ep_a.connect(pk_b, INVITE_ALPN).await {
        Ok(c) => { println!("A connected via DNS"); c }
        Err(_) => {
            println!("A: DNS failed, trying explicit address...");
            let client_addr = ep_b.addr();
            let addr = iroh::EndpointAddr::from_parts(pk_b, client_addr.addrs.clone());
            ep_a.connect(addr, INVITE_ALPN).await?
        }
    };
    println!("A connected to B");

    let payload = InvitePayload {
        org_name: "acme".into(), role: "sales".into(),
        control_ticket: "ctrl-123".into(), data_ticket: "data-456".into(),
    };
    let mut send = conn.open_uni().await?;
    send.write_all(&serde_json::to_vec(&payload)?).await?;
    send.finish()?;
    println!("A sent payload");

    // ── Wait and check queue ──
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let pending = receiver.get_pending();
    println!("B queue: {} pending", pending.len());
    assert_eq!(pending.len(), 1, "One invite should be in queue");
    assert_eq!(pending[0].org_name, "acme");

    // ── Queue should be empty after get_pending (drain) ──
    let after = receiver.get_pending();
    assert_eq!(after.len(), 0, "Queue should be drained after get_pending");

    println!("✓ Invite handler + queue works correctly");
    Ok(())
}
