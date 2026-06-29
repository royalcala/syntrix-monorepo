//! Device-centric namespace registry.
//!
//! Reads `org_<id>/control` and answers:
//! - Is this device active in this org?
//! - What namespaces should it open? (resolves wildcards like `*`)
//! - Can this device write to namespace X?

use std::collections::{HashMap, HashSet};

use iroh_docs::NamespaceId;
use crate::{NodeId, OrgId};

/// A device registered in an org.
#[derive(Debug, Clone)]
pub struct Device {
    pub node_id: NodeId,
    pub active: bool,
    pub role: String,
    pub person: String,
    pub name: String,
}

/// Permissions granted to a role.
#[derive(Debug, Clone, Default)]
pub struct RoleGrants {
    /// Namespace patterns this role can open (supports wildcards: `*`).
    pub can_open: Vec<String>,
    /// Namespaces this role can write to.
    pub can_write: Vec<String>,
}

/// Tracks devices and role grants per organization.
///
/// Populated from `org_<id>/control` entries. Re-synced when control changes.
#[derive(Debug, Default)]
pub struct NamespaceRegistry {
    /// Devices per org.
    devices: HashMap<OrgId, HashMap<NodeId, Device>>,
    /// Role grants per org.
    grants: HashMap<OrgId, HashMap<String, RoleGrants>>,
    /// All known namespace ids per org (for wildcard resolution).
    known_namespaces: HashMap<OrgId, HashSet<String>>,
    /// Maps NamespaceId → OrgId for accept_cb lookup.
    namespace_org_map: HashMap<NamespaceId, OrgId>,
}

impl NamespaceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Population ──

    /// Upsert a device entry.
    pub fn upsert_device(&mut self, org_id: OrgId, node_id: NodeId, device: Device) {
        self.devices.entry(org_id).or_default().insert(node_id, device);
    }

    /// Remove a device from the registry entirely.
    pub fn remove_device(&mut self, org_id: &OrgId, node_id: &NodeId) {
        if let Some(devs) = self.devices.get_mut(org_id) {
            devs.remove(node_id);
        }
    }

    /// Upsert a role grant entry.
    pub fn upsert_role(&mut self, org_id: OrgId, role: String, grants: RoleGrants) {
        self.grants.entry(org_id).or_default().insert(role, grants);
    }

    /// Register known namespaces for an org (used to resolve wildcards).
    pub fn set_known_namespaces(&mut self, org_id: OrgId, namespaces: HashSet<String>) {
        self.known_namespaces.insert(org_id, namespaces);
    }

    /// Add a single known namespace.
    pub fn add_known_namespace(&mut self, org_id: &OrgId, namespace: String) {
        self.known_namespaces
            .entry(org_id.clone())
            .or_default()
            .insert(namespace);
    }

    // ── Queries for accept_cb ──

    /// Check if a device is active in the given org.
    pub fn is_device_active(&self, org_id: &OrgId, node_id: &NodeId) -> bool {
        self.devices
            .get(org_id)
            .and_then(|d| d.get(node_id))
            .map(|d| d.active)
            .unwrap_or(false)
    }

    // ── Queries for namespace opening ──

    /// Get all namespaces this device should open, derived from the role's can_open.
    /// can_open contains entity/namespace names directly (or "*").
    /// When "*" is present, resolves against known_namespaces.
    pub fn openable_namespaces(&self, org_id: &OrgId, node_id: &NodeId) -> HashSet<String> {
        let role = match self.device_role(org_id, node_id) {
            Some(r) => r,
            None => return HashSet::new(),
        };

        let can_open = self
            .grants
            .get(org_id)
            .and_then(|g| g.get(&role))
            .map(|g| g.can_open.clone())
            .unwrap_or_default();

        if can_open.iter().any(|e| e == "*") {
            // Resolve "*" against known_namespaces; fall back to entity names from schema
            if let Some(known) = self.known_namespaces.get(org_id) {
                known.clone()
            } else {
                let registry = syntrix_schema::build_registry();
                registry.entity_names().into_iter().map(|s| s.to_string()).collect()
            }
        } else {
            can_open.into_iter().collect()
        }
    }

    /// Get the device's role in an org.
    fn device_role(&self, org_id: &OrgId, node_id: &NodeId) -> Option<String> {
        self.devices
            .get(org_id)
            .and_then(|d| d.get(node_id))
            .map(|d| d.role.clone())
    }

    // ── Write validation ──

    /// Check if a device can write to the given namespace in this org.
    /// The role's `can_write` contains entity/namespace names (or `"*"`), so we check
    /// directly if the namespace name is in the permission list.
    pub fn can_write(&self, org_id: &OrgId, node_id: &NodeId, namespace: &str) -> bool {
        let role = match self.device_role(org_id, node_id) {
            Some(r) => r,
            None => return false,
        };

        let can_write = match self
            .grants
            .get(org_id)
            .and_then(|g| g.get(&role))
            .map(|g| &g.can_write)
        {
            Some(cw) => cw,
            None => return false,
        };

        can_write.iter().any(|n| n == "*" || n == namespace)
    }

    // ── Multi-org ──

    /// Map a NamespaceId to the org that owns it.
    /// Called when the app opens a namespace (learned from control entries).
    pub fn map_namespace_to_org(&mut self, ns: NamespaceId, org_id: OrgId) {
        self.namespace_org_map.insert(ns, org_id);
    }

    /// Look up which org owns a given namespace.
    pub fn lookup_org(&self, ns: &NamespaceId) -> Option<OrgId> {
        self.namespace_org_map.get(ns).cloned()
    }

    /// List all org IDs known to this registry.
    pub fn org_ids(&self) -> HashSet<OrgId> {
        self.devices.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node_id(seed: u8) -> NodeId {
        let mut id = [0u8; 32];
        id[0] = seed;
        id
    }

    #[test]
    fn test_is_device_active() {
        let mut reg = NamespaceRegistry::new();
        let alice = make_node_id(1);

        reg.upsert_device(
            "acme".into(),
            alice,
            Device {
                node_id: alice,
                active: true,
                role: "sales".into(),
                person: "alice".into(),
                name: "Alice (laptop)".into(),
            },
        );

        assert!(reg.is_device_active(&"acme".into(), &alice));
        assert!(!reg.is_device_active(&"other".into(), &alice));
    }

    #[test]
    fn test_openable_namespaces_via_entities() {
        let mut reg = NamespaceRegistry::new();
        let alice = make_node_id(1);

        reg.upsert_device(
            "acme".into(),
            alice,
            Device {
                node_id: alice,
                active: true,
                role: "sales".into(),
                person: "alice".into(),
                name: "Alice (pc)".into(),
            },
        );

        reg.upsert_role(
            "acme".into(),
            "sales".into(),
            RoleGrants {
                can_open: vec!["customers".into(), "invoices".into()],
                can_write: vec![],
            },
        );

        let openable = reg.openable_namespaces(&"acme".into(), &alice);
        assert!(openable.contains("customers"));
        assert!(openable.contains("invoices"));
        assert!(!openable.contains("payroll"));
        assert!(!openable.contains("suppliers"));
    }

    #[test]
    fn test_can_write() {
        let mut reg = NamespaceRegistry::new();
        let bob = make_node_id(2);

        reg.upsert_device(
            "acme".into(),
            bob,
            Device {
                node_id: bob,
                active: true,
                role: "sales".into(),
                person: "bob".into(),
                name: "Bob".into(),
            },
        );

        reg.upsert_role(
            "acme".into(),
            "sales".into(),
            RoleGrants {
                can_open: vec![],
                can_write: vec!["customers".into(), "invoices".into()],
            },
        );

        assert!(reg.can_write(&"acme".into(), &bob, "customers"));
        assert!(reg.can_write(&"acme".into(), &bob, "invoices"));
        assert!(!reg.can_write(&"acme".into(), &bob, "payroll"));
        assert!(!reg.can_write(&"acme".into(), &bob, "suppliers"));
    }

    #[test]
    fn test_can_write_wildcard() {
        let mut reg = NamespaceRegistry::new();
        let admin = make_node_id(99);

        reg.upsert_device(
            "acme".into(),
            admin,
            Device {
                node_id: admin,
                active: true,
                role: "admin".into(),
                person: "admin".into(),
                name: "Admin".into(),
            },
        );

        reg.upsert_role(
            "acme".into(),
            "admin".into(),
            RoleGrants {
                can_open: vec![],
                can_write: vec!["*".into()],
            },
        );

        assert!(reg.can_write(&"acme".into(), &admin, "org_data"));
        assert!(reg.can_write(&"acme".into(), &admin, "org_private"));
        assert!(reg.can_write(&"acme".into(), &admin, "anything"));
    }
}
