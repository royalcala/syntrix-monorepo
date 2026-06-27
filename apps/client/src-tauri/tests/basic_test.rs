//! Integration tests for syntrix-client headless backend.
//!
//! These tests exercise the extracted `*_impl` functions without Tauri.
//! Run with: `cargo test -p syntrix-client` (via the bridge).

use std::sync::Mutex;
use syntrix_client_lib::*;

/// Verify that `new_with_data_dir` creates an AppState with a valid node_id.
#[tokio::test]
async fn test_new_with_data_dir() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_node");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState::new_with_data_dir should succeed");
    let node_id = state.node_id();
    assert_ne!(node_id, [0u8; 32], "node_id should not be all zeros");
}

/// Test `list_orgs_impl` returns empty for a fresh state.
#[tokio::test]
async fn test_list_orgs_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_list_orgs");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let orgs = list_orgs_impl(&s);
    assert!(orgs.is_empty(), "fresh state should have no orgs");
}

/// Test `sync_status_impl` returns a non-empty status string.
#[tokio::test]
async fn test_sync_status_impl() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_sync_status");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let status = sync_status_impl(&s);
    assert!(status.contains("online"), "status should contain 'online'");
    assert!(status.contains("orgs"), "status should mention org count");
}

/// Test `get_invites_impl` returns empty for a fresh state.
#[tokio::test]
async fn test_get_invites_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_get_invites");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let invites = get_invites_impl(&s);
    assert!(invites.is_empty(), "fresh state should have no invites");
}

/// Test `query_entity_impl` with empty state returns empty results.
#[tokio::test]
async fn test_query_entity_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_query_empty");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let results = query_entity_impl(&s, None, "products", None, None);
    assert!(results.is_ok(), "query should not fail on empty state");
    let results = results.unwrap();
    assert!(results.is_empty(), "empty state should return no results");
}

/// Test `search_entity_impl` returns empty results for empty state.
#[tokio::test]
async fn test_search_entity_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_search_empty");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let results = search_entity_impl(&s, None, "test", None, Some(10));
    assert!(results.is_ok(), "search should not fail on empty state");
    let results = results.unwrap();
    assert!(results.is_empty(), "empty state should return no search results");
}
