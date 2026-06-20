//! QUIC stream invite test: admin endpoint connects to client endpoint,
//! sends invite payload via unidirectional stream, client receives it.

use std::sync::{Arc, Mutex};
use iroh::{Endpoint, endpoint::presets, SecretKey, PublicKey};
use iroh::protocol::{ProtocolHandler, Router, AcceptError};
use iroh::endpoint::Connection;
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
struct Receiver {
    received: Arc<Mutex<Vec<InvitePayload>>>,
}

impl Receiver {
    fn new() -> Self {
        Self { received: Arc::new(Mutex::new(Vec::new())) }
    }
}

impl ProtocolHandler for Receiver {
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

            let payload: InvitePayload = serde_json::from_slice(&buf)
                .map_err(|e| AcceptError::from_boxed(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))))?;

            received.lock().unwrap().push(payload);
            Ok(())
        }
    }
}

#[tokio::test]
async fn test_quic_invite_stream() -> anyhow::Result<()> {
    // ── Create two endpoints ──
    let secret_a = SecretKey::from_bytes(&[1u8; 32]);
    let secret_b = SecretKey::from_bytes(&[2u8; 32]);
    let pk_b: PublicKey = secret_b.public();

    let ep_a = Endpoint::builder(presets::N0)
        .secret_key(secret_a)
        .bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>()?)?
        .bind().await?;

    let ep_b = Endpoint::builder(presets::N0)
        .secret_key(secret_b)
        .bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>()?)?
        .bind().await?;

    // Both nodes need to be online for relay discovery
    ep_a.online().await;
    ep_b.online().await;

    let addr_b = ep_b.addr();
    println!("A: {}  B: {}  B addrs: {:?}", ep_a.id().fmt_short(), ep_b.id().fmt_short(), addr_b.addrs);

    // ── B registers the invite handler ──
    let receiver = Receiver::new();
    let _router_b = Router::builder(ep_b.clone())
        .accept(INVITE_ALPN, receiver.clone())
        .spawn();

    // ── A connects to B and sends invite ──
    // Try connecting with just the PublicKey (relay-based discovery)
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let conn = match ep_a.connect(pk_b, INVITE_ALPN).await {
        Ok(c) => {
            println!("A connected to B via relay");
            c
        }
        Err(e) => {
            println!("A failed to connect via PublicKey: {e}. Trying with explicit address...");
            // Fallback: use explicit local address
            let addr_b_explicit = iroh::EndpointAddr::from_parts(
                ep_b.id(),
                addr_b.addrs.clone(),
            );
            ep_a.connect(addr_b_explicit, INVITE_ALPN).await?
        }
    };

    // Send the invite payload
    let payload = InvitePayload {
        org_name: "acme".into(),
        role: "sales".into(),
        control_ticket: "ticket-ctrl-123".into(),
        data_ticket: "ticket-data-456".into(),
    };
    let json = serde_json::to_vec(&payload)?;

    let mut send = conn.open_uni().await?;
    send.write_all(&json).await?;
    send.finish()?;
    println!("A sent invite");

    // Wait a bit for B to process
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // ── Verify B received it ──
    let received = receiver.received.lock().unwrap();
    assert_eq!(received.len(), 1, "B should receive exactly 1 invite");
    assert_eq!(received[0], payload, "B should receive the correct payload");

    println!("✓ QUIC invite stream works: A → B via localhost");
    Ok(())
}
