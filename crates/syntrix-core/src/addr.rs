use libp2p::{Multiaddr, PeerId};

/// Parse a device address string into an optional `PeerId`.
///
/// Accepts three formats:
/// - JSON object with `node_id` and optional `addrs` array.
/// - A `Multiaddr` string (e.g. `/ip4/.../udp/.../quic-v1/p2p/<peer_id>`).
/// - A bare hex-encoded `PeerId` (64 hex chars).
pub fn parse_device_addr(addr_str: &str) -> Option<PeerId> {
    if addr_str.is_empty() {
        return None;
    }

    // 1. JSON format: {"node_id": "...", "addrs": [...]} — extract node_id as hex → PeerId
    if addr_str.trim().starts_with('{') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(addr_str) {
            let node_id_hex = val["node_id"].as_str()?;
            return parse_hex_peer_id(node_id_hex);
        }
    }

    // 2. Try parsing as Multiaddr (may contain /p2p/<peer_id> at the end)
    if let Ok(ma) = addr_str.parse::<Multiaddr>() {
        if let Some(peer_id) = ma.iter().last().and_then(|p| {
            if let libp2p::multiaddr::Protocol::P2p(pid) = p {
                Some(pid)
            } else {
                None
            }
        }) {
            return Some(peer_id);
        }
        // If no /p2p/ component, try using the whole addr as peer ID
    }

    // 3. Bare hex node ID (64 hex chars) → PeerId
    parse_hex_peer_id(addr_str)
}

fn parse_hex_peer_id(hex_str: &str) -> Option<PeerId> {
    let bytes = hex::decode(hex_str).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    PeerId::from_bytes(&bytes).ok()
}

/// Build a device address string from a peer ID and a list of listen addresses.
///
/// Returns a JSON string `{"node_id": "<hex>", "addrs": ["<multiaddr_str>", ...]}`.
pub fn build_device_addr_string(peer_id: PeerId, addrs: &[Multiaddr]) -> String {
    let addr_strs: Vec<String> = addrs
        .iter()
        .map(|a| a.to_string())
        .collect();
    serde_json::json!({
        "node_id": hex::encode(peer_id.to_bytes()),
        "addrs": addr_strs,
    })
    .to_string()
}
