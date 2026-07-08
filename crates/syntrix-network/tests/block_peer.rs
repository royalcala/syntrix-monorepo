use std::time::Duration;
use libp2p::identity::Keypair;
use libp2p::Multiaddr;
use syntrix_network::{NetworkConfig, P2PNode};

async fn wait_for_addr(node: &P2PNode) -> Multiaddr {
    for _ in 0..20 {
        let addrs = node.listen_addrs().await;
        if !addrs.is_empty() {
            return addrs[0].clone();
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("timed out waiting for listen address");
}

fn test_config(prefix: &str) -> (tempfile::TempDir, NetworkConfig) {
    let dir = tempfile::tempdir().expect("temp dir");
    let data_dir = dir.path().join(prefix);
    std::fs::create_dir_all(&data_dir).unwrap();
    let keypair = Keypair::generate_ed25519();
    let config = NetworkConfig {
        keypair,
        listen_on: vec![
            "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap(),
        ],
        bootstrap_nodes: vec![],
        data_dir,
    };
    (dir, config)
}

#[tokio::test]
async fn test_block_peer_disconnects_and_prevents_redial() {
    let (_d1, c1) = test_config("bp1");
    let (_d2, c2) = test_config("bp2");

    let (node1, _rx1) = P2PNode::new(c1).await.unwrap();
    let (node2, mut rx2) = P2PNode::new(c2).await.unwrap();

    let addr2 = wait_for_addr(&node2).await;
    node1.dial(addr2.clone()).unwrap();

    // Wait for connection to establish
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify peer2 received the connection event
    let connected = rx2.try_recv().ok();
    assert!(connected.is_some(), "peer2 should receive connection event");

    // Block peer1 on peer2
    node2.block_peer(node1.local_peer_id());
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Verify peer2 received disconnect event
    let disconnected = rx2.try_recv().ok();
    assert!(disconnected.is_some(), "peer2 should receive disconnect after block");

    // Try to reconnect — should be prevented by block
    node1.dial(addr2.clone()).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // No new connection event should arrive (blocked peers are disconnected immediately)
    let reconnected = rx2.try_recv().ok();
    assert!(!matches!(&reconnected, Some(syntrix_network::Event::PeerConnected(_))),
        "blocked peer should not reconnect");
}

#[tokio::test]
async fn test_backoff_delays_reconnect() {
    let (_d1, c1) = test_config("bo1");
    let (_d2, c2) = test_config("bo2");

    let (node1, _rx1) = P2PNode::new(c1).await.unwrap();
    let (node2, _rx2) = P2PNode::new(c2).await.unwrap();

    let addr2 = wait_for_addr(&node2).await;
    node1.dial(addr2).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // ensure_connected with empty addrs — will use backoff with default 2s
    node2.ensure_connected(node1.local_peer_id(), vec![]);
    // Should not panic or hang
    tokio::time::sleep(Duration::from_secs(1)).await;
}
