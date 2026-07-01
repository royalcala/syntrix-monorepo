use std::collections::{HashMap, HashSet};

use crate::{NodeId, OrgId, ENTITY_NAMES};

pub fn can_access(allowed: &[String], entity: &str) -> bool {
    allowed.iter().any(|a| a == entity || a == "*")
}

#[derive(Debug, Clone)]
pub struct Device {
    pub node_id: NodeId,
    pub active: bool,
    pub role: String,
    pub person: String,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct RoleGrants {
    pub can_open: Vec<String>,
    pub can_write: Vec<String>,
}

#[derive(Debug, Default)]
pub struct NamespaceRegistry {
    devices: HashMap<OrgId, HashMap<NodeId, Device>>,
    grants: HashMap<OrgId, HashMap<String, RoleGrants>>,
    known_namespaces: HashMap<OrgId, HashSet<String>>,
    topic_ids: HashMap<OrgId, String>,
}

impl NamespaceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_device(&mut self, org_id: OrgId, node_id: NodeId, device: Device) {
        self.devices.entry(org_id).or_default().insert(node_id, device);
    }

    pub fn remove_device(&mut self, org_id: &OrgId, node_id: &NodeId) {
        if let Some(devs) = self.devices.get_mut(org_id) {
            devs.remove(node_id);
        }
    }

    pub fn upsert_role(&mut self, org_id: OrgId, role: String, grants: RoleGrants) {
        self.grants.entry(org_id).or_default().insert(role, grants);
    }

    pub fn set_known_namespaces(&mut self, org_id: OrgId, namespaces: HashSet<String>) {
        self.known_namespaces.insert(org_id, namespaces);
    }

    pub fn add_known_namespace(&mut self, org_id: &OrgId, namespace: String) {
        self.known_namespaces
            .entry(org_id.clone())
            .or_default()
            .insert(namespace);
    }

    pub fn is_device_active(&self, org_id: &OrgId, node_id: &NodeId) -> bool {
        self.devices
            .get(org_id)
            .and_then(|d| d.get(node_id))
            .map(|d| d.active)
            .unwrap_or(false)
    }

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
            if let Some(known) = self.known_namespaces.get(org_id) {
                known.clone()
            } else {
                ENTITY_NAMES.iter().map(|s| s.to_string()).collect()
            }
        } else {
            can_open.into_iter().collect()
        }
    }

    fn device_role(&self, org_id: &OrgId, node_id: &NodeId) -> Option<String> {
        self.devices
            .get(org_id)
            .and_then(|d| d.get(node_id))
            .map(|d| d.role.clone())
    }

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

    pub fn set_topic_id(&mut self, org_id: OrgId, topic_id: String) {
        self.topic_ids.insert(org_id, topic_id);
    }

    pub fn get_topic_id(&self, org_id: &OrgId) -> Option<String> {
        self.topic_ids.get(org_id).cloned()
    }

    pub fn map_topic_to_org(&mut self, topic_id: String, org_id: OrgId) {
        self.topic_ids.insert(org_id, topic_id);
    }

    pub fn lookup_org_by_topic(&self, topic_id: &str) -> Option<OrgId> {
        self.topic_ids.iter().find(|(_, t)| t.as_str() == topic_id).map(|(org, _)| org.clone())
    }

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

    #[test]
    fn test_topic_id_mapping() {
        let mut reg = NamespaceRegistry::new();
        let topic = "syntrix-org-acme".to_string();
        reg.set_topic_id("acme".into(), topic.clone());
        assert_eq!(reg.get_topic_id(&"acme".into()), Some(topic.clone()));
        assert_eq!(reg.lookup_org_by_topic(&topic), Some("acme".into()));
    }
}
