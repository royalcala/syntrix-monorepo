use std::sync::Arc;

use syntrix_network::P2PNode;

pub const CATCHUP_ALPN: &[u8] = b"/syntrix/catchup/1";

/// Admin catchup client: request missed events from a peer after restart.
pub async fn admin_catchup_from_peer(
    p2p: &P2PNode,
    peer_id: libp2p::PeerId,
    org_id: &str,
    db: &Arc<turso_core::Connection>,
) -> anyhow::Result<()> {
    let events = p2p.request_catchup(peer_id, org_id.to_string(), 0).await?;
    let count = events.len();
    for event in &events {
        crate::gossip::write_event_to_limbo(db, org_id, event);
    }

    tracing::info!(
        org = %org_id,
        peer = %peer_id,
        count = %count,
        "admin-catchup: received events from peer"
    );

    Ok(())
}
