//! Integration tests for syntrix-admin headless backend.
//!
//! Run with: `cargo test -p syntrix-admin` (via the bridge).

use std::time::Duration;

use syntrix_admin_lib::*;

/// Verify that `new_with_data_dir` creates an admin AppState.
#[tokio::test]
async fn test_admin_new_with_data_dir() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("admin_test_node");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("Admin AppState::new_with_data_dir should succeed");
    let node_id = state.node_id();
    assert_ne!(node_id, [0u8; 32], "node_id should not be all zeros");
}

/// Verify `list_orgs` is empty for a fresh admin state.
#[tokio::test]
async fn test_admin_list_orgs_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("admin_list_orgs");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("Admin AppState init");
    let orgs = state.list_orgs();
    assert!(orgs.is_empty(), "fresh admin state should have no orgs");
}

/// Verify `node_id()` returns a valid hex node ID.
#[tokio::test]
async fn test_admin_node_id() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("admin_node_id");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("Admin AppState init");
    let hex_id = hex::encode(state.node_id());
    assert_eq!(hex_id.len(), 64, "hex node_id should be 64 chars");
    assert!(!hex_id.chars().all(|c| c == '0'), "node_id should not be all zeros");
}

/// Verify `data_dir()` returns the correct path.
#[tokio::test]
async fn test_admin_data_dir() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("admin_data_dir");
    let state = identity::AppState::new_with_data_dir(data_dir.clone())
        .await
        .expect("Admin AppState init");
    assert_eq!(state.data_dir(), &data_dir, "data_dir should match the provided path");
}

/// Verify that after `create_org`, the admin heartbeat fires and `get_sync_info`
/// shows the admin itself as "online".
#[tokio::test]
async fn test_admin_create_org_gossip_and_heartbeat() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("admin_org_hb");
    let mut state = identity::AppState::new_with_data_dir(data_dir.clone())
        .await
        .expect("Admin AppState init");

    // 1. Create an org — this should join gossip + start heartbeat.
    admin::create_org(&mut state, "testorg")
        .await
        .expect("create_org");

    let node_id_hex = hex::encode(state.node_id());

    // 2. Poll until the admin's own heartbeat appears (first iteration fires immediately).
    let found = syntrix_testkit::poll_until(
        || {
            let hb = state.get_heartbeats("testorg");
            let ts = hb.get(&node_id_hex).copied().unwrap_or(0);
            async move {
                if ts > 0 {
                    Ok(true)
                } else {
                    Err("no heartbeat yet")
                }
            }
        },
        &syntrix_testkit::PollConfig {
            max_retries: 50,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
        },
    )
    .await
    .expect("admin heartbeat should appear within 5 seconds");

    assert!(found, "admin heartbeat was recorded");

    // 3. `get_sync_info` must show the admin as "online".
    let info: admin::SyncInfo = admin::get_sync_info(&state, "testorg");
    let admin_peer = info
        .peers
        .iter()
        .find(|p| p.node_id == node_id_hex)
        .expect("admin should appear in sync-info peer list");
    assert_eq!(admin_peer.status, "online", "admin should show as online");

    // 4. Org should be persisted to disk (devices.json, roles.json).
    assert!(
        data_dir.join("devices.json").exists(),
        "devices.json should be persisted"
    );
    assert!(
        data_dir.join("roles.json").exists(),
        "roles.json should be persisted"
    );
}
