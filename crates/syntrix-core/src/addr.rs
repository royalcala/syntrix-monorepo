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

    // 1. JSON format: {"node_id": "...", "addrs": [...]} — extract node_id as hex → PeerId.
    // `build_device_addr_string` encodes the FULL `PeerId::to_bytes()` (protobuf/multihash
    // form, variable length), not the 32-byte raw node id used elsewhere (heartbeats,
    // members table) — so this branch must decode via `PeerId::from_bytes` directly rather
    // than `parse_hex_peer_id` (which is for the 32-byte raw form, see below).
    if addr_str.trim().starts_with('{') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(addr_str) {
            let node_id_hex = val["node_id"].as_str()?;
            let bytes = hex::decode(node_id_hex).ok()?;
            return PeerId::from_bytes(&bytes).ok();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test: `build_device_addr_string`'s output must always round-trip through
    /// `parse_device_addr` back to the same `PeerId`. This was silently broken before (the
    /// JSON branch required exactly 32 raw bytes, but `build_device_addr_string` encodes the
    /// full protobuf-encoded `PeerId::to_bytes()`), which meant `admin_addr`-based catchup
    /// (join_org_impl, identity.rs restart catchup) always failed and every peer only ever
    /// learned about itself in `members`/`roles` — breaking the CDC author permission check
    /// (Fase 3, tarea 15) for any event authored by a different peer.
    #[test]
    fn build_then_parse_device_addr_roundtrips() {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = keypair.public().to_peer_id();
        let addrs: Vec<Multiaddr> = vec!["/ip4/127.0.0.1/udp/4001/quic-v1".parse().unwrap()];

        let addr_str = build_device_addr_string(peer_id, &addrs);
        let parsed = parse_device_addr(&addr_str).expect("must parse back a PeerId");
        assert_eq!(parsed, peer_id);
    }

    #[test]
    fn parse_device_addr_handles_multiaddr_with_p2p_suffix() {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = keypair.public().to_peer_id();
        let addr_str = format!("/ip4/127.0.0.1/udp/4001/quic-v1/p2p/{peer_id}");
        assert_eq!(parse_device_addr(&addr_str), Some(peer_id));
    }

    #[test]
    fn parse_device_addr_rejects_garbage() {
        assert_eq!(parse_device_addr(""), None);
        assert_eq!(parse_device_addr("not-an-address"), None);
        assert_eq!(parse_device_addr("{}"), None);
    }
}
