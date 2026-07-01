pub mod addr;
pub mod heartbeat;
pub mod sync;
pub mod registry;

pub use addr::{parse_device_addr, build_device_addr_string};
pub use heartbeat::start_heartbeat_with_resync;
pub use sync::{PeerStatus, SyncInfo, get_sync_info};
pub use registry::{NamespaceRegistry, Device, RoleGrants, can_access};

pub type OrgId = String;
pub type NodeId = [u8; 32];

pub const ENTITY_NAMES: &[&str] = &["customers", "suppliers", "products", "invoices", "orders", "payroll"];

pub fn schema_version_for(_entity: &str) -> u32 { 1 }

pub fn upcast_payload(payload: serde_json::Value, _from_schema: u32, _to_schema: u32) -> serde_json::Value {
    payload
}
