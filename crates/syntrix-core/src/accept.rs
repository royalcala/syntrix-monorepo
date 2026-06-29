//! Multi-org accept callback for sync connections.
//!
//! Consults `NamespaceRegistry` to find which org owns the namespace being synced,
//! then checks if the connecting device is active in that org.

use iroh::PublicKey;
use iroh_docs::{net::AcceptOutcome, net::AbortReason, NamespaceId};

use crate::registry::NamespaceRegistry;

/// Creates a multi-org accept callback compatible with iroh-docs `AcceptCallback`.
///
/// The registry is shared via `Arc<RwLock<>>` so it can be updated by admin commands
/// while being read by the accept callback.
pub fn make_accept_cb(
    registry: std::sync::Arc<std::sync::RwLock<NamespaceRegistry>>,
) -> std::sync::Arc<
    dyn Fn(NamespaceId, PublicKey) -> std::future::Ready<AcceptOutcome> + Send + Sync + 'static,
> {
    std::sync::Arc::new(
        move |namespace: NamespaceId, peer: PublicKey| {
            let peer_bytes: &[u8; 32] = peer.as_bytes();

            let outcome = match registry.read() {
                Ok(reg) => match reg.lookup_org(&namespace) {
                    Some(org_id) => {
                        if reg.is_device_active(&org_id, peer_bytes) {
                            // Peer is registered and active → allow
                            AcceptOutcome::Allow
                        } else if !reg.has_devices_for_org(&org_id) {
                            // Bootstrap mode: namespace is known but no devices
                            // have been synced yet (fresh data directory). Allow
                            // incoming sync so the control doc data can be
                            // populated. Once at least one peer synced, the
                            // heartbeat loop will load members into the registry
                            // and the normal device-based check takes over.
                            AcceptOutcome::Allow
                        } else {
                            AcceptOutcome::Reject(AbortReason::NotFound)
                        }
                    }
                    None => AcceptOutcome::Reject(AbortReason::NotFound),
                },
                Err(_) => AcceptOutcome::Reject(AbortReason::NotFound),
            };

            std::future::ready(outcome)
        }
    )
}

#[cfg(test)]
mod tests {
    use iroh::SecretKey;

    use super::*;
    use crate::registry::Device;

    fn make_pubkey(seed: u8) -> PublicKey {
        let secret = SecretKey::from_bytes(&[seed; 32]);
        secret.public()
    }

    fn make_namespace_id(seed: u8) -> NamespaceId {
        let mut bytes = [0u8; 32];
        bytes[seed as usize % 32] = seed;
        NamespaceId::from(&bytes)
    }

    #[test]
    fn test_accept_active_device() {
        let mut reg = NamespaceRegistry::new();
        let alice_key = make_pubkey(1);
        let alice_id = *alice_key.as_bytes();
        let ns = make_namespace_id(10);

        reg.upsert_device(
            "acme".into(),
            alice_id,
            Device {
                node_id: alice_id,
                active: true,
                role: "sales".into(),
                person: "alice".into(),
                name: "Alice".into(),
            },
        );
        reg.map_namespace_to_org(ns, "acme".into());

        let cb = make_accept_cb(std::sync::Arc::new(std::sync::RwLock::new(reg)));
        let outcome = cb(ns, alice_key).into_inner();
        assert!(matches!(outcome, AcceptOutcome::Allow));
    }

    #[test]
    fn test_reject_inactive_device() {
        let mut reg = NamespaceRegistry::new();
        let bob_key = make_pubkey(2);
        let bob_id = *bob_key.as_bytes();
        let ns = make_namespace_id(20);

        reg.upsert_device(
            "acme".into(),
            bob_id,
            Device {
                node_id: bob_id,
                active: false,
                role: "sales".into(),
                person: "bob".into(),
                name: "Bob".into(),
            },
        );
        reg.map_namespace_to_org(ns, "acme".into());

        let cb = make_accept_cb(std::sync::Arc::new(std::sync::RwLock::new(reg)));
        let outcome = cb(ns, bob_key).into_inner();
        assert!(matches!(outcome, AcceptOutcome::Reject(_)));
    }

    #[test]
    fn test_reject_unknown_device() {
        let mut reg = NamespaceRegistry::new();
        let unknown_key = make_pubkey(99);
        let ns = make_namespace_id(30);

        reg.map_namespace_to_org(ns, "acme".into());

        let cb = make_accept_cb(std::sync::Arc::new(std::sync::RwLock::new(reg)));
        let outcome = cb(ns, unknown_key).into_inner();
        assert!(matches!(outcome, AcceptOutcome::Reject(_)));
    }

    #[test]
    fn test_reject_unmapped_namespace() {
        let mut reg = NamespaceRegistry::new();
        let alice_key = make_pubkey(1);
        let alice_id = *alice_key.as_bytes();
        let ns = make_namespace_id(40);

        reg.upsert_device(
            "acme".into(),
            alice_id,
            Device {
                node_id: alice_id,
                active: true,
                role: "sales".into(),
                person: "alice".into(),
                name: "Alice".into(),
            },
        );
        // NOT mapping ns to org → should reject

        let cb = make_accept_cb(std::sync::Arc::new(std::sync::RwLock::new(reg)));
        let outcome = cb(ns, alice_key).into_inner();
        assert!(matches!(outcome, AcceptOutcome::Reject(_)));
    }

    #[test]
    fn test_accept_bootstrap_unknown_peer() {
        // Bootstrap: namespace mapped but no devices registered yet.
        // Unknown peer should be ALLOWED so the control doc can sync.
        let mut reg = NamespaceRegistry::new();
        let unknown_key = make_pubkey(99);
        let ns = make_namespace_id(70);

        reg.map_namespace_to_org(ns, "acme".into());
        // NOT adding any devices — simulates fresh data dir

        let cb = make_accept_cb(std::sync::Arc::new(std::sync::RwLock::new(reg)));
        let outcome = cb(ns, unknown_key).into_inner();
        assert!(
            matches!(outcome, AcceptOutcome::Allow),
            "bootstrap should allow unknown peer when no devices registered"
        );
    }

    #[test]
    fn test_reject_unknown_peer_after_bootstrap() {
        // After bootstrap: devices exist but this specific peer is unknown.
        // Should be REJECTED.
        let mut reg = NamespaceRegistry::new();
        let known_key = make_pubkey(1);
        let known_id = *known_key.as_bytes();
        let unknown_key = make_pubkey(99);
        let ns = make_namespace_id(80);

        reg.map_namespace_to_org(ns, "acme".into());
        reg.upsert_device(
            "acme".into(),
            known_id,
            Device {
                node_id: known_id,
                active: true,
                role: "sales".into(),
                person: "alice".into(),
                name: "Alice".into(),
            },
        );

        let cb = make_accept_cb(std::sync::Arc::new(std::sync::RwLock::new(reg)));
        let outcome = cb(ns, unknown_key).into_inner();
        assert!(
            matches!(outcome, AcceptOutcome::Reject(_)),
            "unknown peer should be rejected after bootstrap (devices exist)"
        );
    }

    #[test]
    fn test_multi_org_accept() {
        let mut reg = NamespaceRegistry::new();
        let alice_key = make_pubkey(1);
        let alice_id = *alice_key.as_bytes();
        let bob_key = make_pubkey(2);
        let bob_id = *bob_key.as_bytes();

        let ns_acme = make_namespace_id(50);
        let ns_clientex = make_namespace_id(60);

        // Alice active in acme only
        reg.upsert_device(
            "acme".into(),
            alice_id,
            Device {
                node_id: alice_id,
                active: true,
                role: "sales".into(),
                person: "alice".into(),
                name: "Alice".into(),
            },
        );
        reg.map_namespace_to_org(ns_acme, "acme".into());

        // Bob active in clientex only
        reg.upsert_device(
            "clientex".into(),
            bob_id,
            Device {
                node_id: bob_id,
                active: true,
                role: "contabilidad".into(),
                person: "bob".into(),
                name: "Bob".into(),
            },
        );
        reg.map_namespace_to_org(ns_clientex, "clientex".into());

        let cb = make_accept_cb(std::sync::Arc::new(std::sync::RwLock::new(reg)));

        // Alice can access acme
        assert!(matches!(cb(ns_acme, alice_key).into_inner(), AcceptOutcome::Allow));
        // Alice cannot access clientex
        assert!(matches!(cb(ns_clientex, alice_key).into_inner(), AcceptOutcome::Reject(_)));
        // Bob can access clientex
        assert!(matches!(cb(ns_clientex, bob_key).into_inner(), AcceptOutcome::Allow));
        // Bob cannot access acme
        assert!(matches!(cb(ns_acme, bob_key).into_inner(), AcceptOutcome::Reject(_)));
    }
}
