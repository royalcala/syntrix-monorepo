//! Integration tests for syntrix-admin headless backend.
//!
//! Run with: `cargo test -p syntrix-admin` (via the bridge).

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
