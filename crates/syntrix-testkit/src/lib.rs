//! Headless test harness helpers for syntrix-admin and syntrix-client.
//!
//! Provides utilities for constructing multi-node test scenarios without
//! requiring Tauri or GUI interaction.

use std::path::PathBuf;
use std::time::Duration;

/// Polling backoff configuration for async assertions.
pub struct PollConfig {
    pub max_retries: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            max_retries: 30,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(5),
        }
    }
}

/// Create a temporary directory for isolated test node data.
pub fn temp_node_dir(prefix: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join(prefix);
    std::fs::create_dir_all(&path).ok();
    (dir, path)
}

/// Async poll with exponential backoff until `f` returns `Ok(value)` or timeout.
pub async fn poll_until<T, E, F, Fut>(mut f: F, config: &PollConfig) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut delay = config.base_delay;
    for attempt in 0..config.max_retries {
        match f().await {
            Ok(value) => return Ok(value),
            Err(_) if attempt + 1 < config.max_retries => {
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(config.max_delay);
            }
            Err(_) => return Err(format!("poll_until exhausted after {} attempts", config.max_retries)),
        }
    }
    Err("poll_until: unreachable".into())
}

/// Generate an invite-style ticket string for a given namespace.
/// This mirrors the admin's `send_invite` logic for test setup without
/// depending on the admin crate.
pub async fn make_invite_ticket(
    doc: &iroh_docs::api::Doc,
) -> anyhow::Result<String> {
    let ticket = doc
        .share(
            iroh_docs::api::protocol::ShareMode::Write,
            iroh_docs::api::protocol::AddrInfoOptions::RelayAndAddresses,
        )
        .await?;
    Ok(ticket.to_string())
}

/// Parse a hex node ID string into a `[u8; 32]` byte array.
pub fn parse_node_id(hex_str: &str) -> anyhow::Result<[u8; 32]> {
    let bytes = hex::decode(hex_str)?;
    if bytes.len() != 32 {
        anyhow::bail!("node_id must be 32 bytes, got {}", bytes.len());
    }
    let mut id = [0u8; 32];
    id.copy_from_slice(&bytes);
    Ok(id)
}

/// An invite payload struct matching `InvitePayload` in client and admin apps.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct TestInvitePayload {
    pub org_name: String,
    pub role: String,
    pub tickets: Vec<TestTicketInfo>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct TestTicketInfo {
    pub ns: String,
    pub ticket: String,
}

/// Wait for a sync condition with a generous timeout.
pub async fn wait_for_sync(timeout_secs: u64) {
    tokio::time::sleep(Duration::from_secs(timeout_secs)).await;
}
