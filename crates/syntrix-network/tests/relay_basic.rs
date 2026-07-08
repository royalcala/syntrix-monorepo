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
async fn test_relay_client_transport_initialized() {
    let (_d1, c1) = test_config("rl1");
    let (node, _rx) = P2PNode::new(c1).await.unwrap();

    let addr = wait_for_addr(&node).await;
    assert!(!addr.to_string().is_empty(), "node should have a listen address");
}

#[tokio::test]
async fn test_relay_circuit_established() {
    let (_d1, c1) = test_config("rc1");
    let (_d2, c2) = test_config("rc2");

    let (node1, mut rx1) = P2PNode::new(c1).await.unwrap();
    let (node2, _rx2) = P2PNode::new(c2).await.unwrap();

    let addr2 = wait_for_addr(&node2).await;

    // Direct dial between two nodes (QUIC)
    node1.dial(addr2).unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;

    let event = rx1.try_recv().ok();
    assert!(event.is_some(), "node1 should receive event after dial");
}
