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
async fn test_reconnect_after_block() {
    let (_d1, c1) = test_config("rc1");
    let (_d2, c2) = test_config("rc2");

    let (node1, mut rx1) = P2PNode::new(c1).await.unwrap();
    let (node2, mut rx2) = P2PNode::new(c2).await.unwrap();

    let addr2 = wait_for_addr(&node2).await;

    node1.dial(addr2.clone()).unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;

    let connected1 = rx1.try_recv().ok();
    assert!(connected1.is_some(), "node1 should receive connection event");

    // Block peer1 on peer2 to force disconnect
    node2.block_peer(node1.local_peer_id());
    tokio::time::sleep(Duration::from_secs(2)).await;

    let disconnected = rx1.try_recv().ok();
    assert!(disconnected.is_some(), "node1 should detect disconnect");

    // Unblock and re-dial
    node2.unblock_peer(node1.local_peer_id());
    node1.dial(addr2.clone()).unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;

    let reconnected = rx1.try_recv().ok();
    assert!(reconnected.is_some(), "node1 should reconnect after unblock");
}

#[tokio::test]
async fn test_reconnect_backoff_with_bad_addr() {
    let (_d1, c1) = test_config("rb1");
    let (_d2, c2) = test_config("rb2");

    let (node1, _rx1) = P2PNode::new(c1).await.unwrap();
    let (node2, _rx2) = P2PNode::new(c2).await.unwrap();

    let _addr2 = wait_for_addr(&node2).await;

    // ensure_connected with unreachable address — should backoff, not panic
    let bad_addr: libp2p::Multiaddr = "/ip4/127.0.0.1/udp/19999/quic-v1".parse().unwrap();
    node1.ensure_connected(node2.local_peer_id(), vec![bad_addr]);
    tokio::time::sleep(Duration::from_secs(2)).await;
}
