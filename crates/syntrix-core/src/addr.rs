//! Address parsing and building utilities for iroh endpoints.
//!
//! These functions handle the multiple wire-formats in which a device address
//! can be stored in the control document:
//!   1. JSON object: `{"node_id": "<hex>", "addrs": ["relay:<url>", "ip:<addr>"]}`
//!   2. Semicolon-separated: `<node_id_hex>;relay:<url>;ip:<addr>`
//!   3. Bare hex node ID (relay-only discovery)

/// Parse a device address string into an `iroh::EndpointAddr`.
///
/// Accepts three formats:
/// - JSON object with `node_id` and optional `addrs` array.
/// - Semicolon-separated `<hex_node_id>;<addr1>;<addr2>` (legacy).
/// - Bare 64-char hex node ID (relay-only, no direct addresses).
pub fn parse_device_addr(addr_str: &str) -> Option<iroh::EndpointAddr> {
    if addr_str.is_empty() {
        return None;
    }

    // 1. JSON format: {"node_id": "...", "addrs": [...]}
    if addr_str.trim().starts_with('{') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(addr_str) {
            let node_id_hex = val["node_id"].as_str()?;
            let node_id_bytes = hex::decode(node_id_hex).ok()?;
            let node_id: [u8; 32] = node_id_bytes.as_slice().try_into().ok()?;
            let peer = iroh::PublicKey::from_bytes(&node_id).ok()?;

            let addrs: Vec<iroh::TransportAddr> = val["addrs"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| {
                            let s = v.as_str()?;
                            if let Some(relay_str) = s.strip_prefix("relay:") {
                                relay_str
                                    .parse::<iroh::RelayUrl>()
                                    .ok()
                                    .map(iroh::TransportAddr::Relay)
                            } else {
                                let addr_str = s.strip_prefix("ip:").unwrap_or(s);
                                addr_str
                                    .parse::<std::net::SocketAddr>()
                                    .ok()
                                    .map(iroh::TransportAddr::Ip)
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            return Some(iroh::EndpointAddr::from_parts(peer, addrs));
        }
    }

    // 2. Semicolon-separated format
    if addr_str.contains(';') {
        let parts: Vec<&str> = addr_str.split(';').collect();
        let node_id_bytes = hex::decode(parts[0]).ok()?;
        let node_id: [u8; 32] = node_id_bytes.as_slice().try_into().ok()?;
        let peer = iroh::PublicKey::from_bytes(&node_id).ok()?;

        let addrs: Vec<iroh::TransportAddr> = parts[1..]
            .iter()
            .filter_map(|s| {
                if let Some(relay_str) = s.strip_prefix("relay:") {
                    relay_str
                        .parse::<iroh::RelayUrl>()
                        .ok()
                        .map(iroh::TransportAddr::Relay)
                } else {
                    let s_addr = s.strip_prefix("ip:").unwrap_or(s);
                    s_addr
                        .parse::<std::net::SocketAddr>()
                        .ok()
                        .map(iroh::TransportAddr::Ip)
                }
            })
            .collect();

        return Some(iroh::EndpointAddr::from_parts(peer, addrs));
    }

    // 3. Bare hex node ID — no direct addresses, rely on relay/DNS discovery
    let node_id_bytes = hex::decode(addr_str).ok()?;
    let node_id: [u8; 32] = node_id_bytes.as_slice().try_into().ok()?;
    let peer = iroh::PublicKey::from_bytes(&node_id).ok()?;
    Some(iroh::EndpointAddr::from_parts(peer, []))
}

/// Build a serializable device address string from an endpoint's current network state.
///
/// Format: `<node_id_hex>;<relay_or_ip_addr1>;<relay_or_ip_addr2>;...`
/// If no addresses are available, returns just the hex node ID.
pub fn build_device_addr_string(endpoint: &iroh::Endpoint) -> String {
    let addr = endpoint.addr();
    let node_id_hex = hex::encode(addr.id.as_bytes());
    let addrs: Vec<String> = addr
        .addrs
        .iter()
        .filter_map(|a| match a {
            iroh::TransportAddr::Relay(url) => Some(format!("relay:{}", url)),
            iroh::TransportAddr::Ip(sa) => Some(format!("ip:{}", sa)),
            _ => None,
        })
        .collect();

    if addrs.is_empty() {
        node_id_hex
    } else {
        format!("{};{}", node_id_hex, addrs.join(";"))
    }
}
