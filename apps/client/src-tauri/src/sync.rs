use crate::identity::AppState;

pub use syntrix_core::{SyncInfo, get_sync_info};

pub fn sync_status(state: &AppState) -> String {
    format!("online · {} orgs · node {}", state.list_orgs().len(), hex::encode(state.node_id())[..8].to_string())
}

pub async fn get_sync_info_impl(
    state: &AppState,
    org: &str,
) -> anyhow::Result<SyncInfo> {
    let members = state.indexer().get_members(org)?;
    let heartbeats = state.indexer().get_heartbeats(org)?;
    Ok(syntrix_core::sync::get_sync_info(members, heartbeats, state.node_id()))
}
