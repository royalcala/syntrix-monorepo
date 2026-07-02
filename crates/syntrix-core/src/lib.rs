pub mod addr;
pub mod heartbeat;
pub mod sync;
pub mod registry;
// The column registry lives in `syntrix-network` (crates/syntrix-network/src/schema.rs) so
// that `syntrix-network::cdc` can use it without a circular dependency on this crate
// (syntrix-core already depends on syntrix-network). Re-exported here so existing call sites
// (e.g. apps/client/src-tauri/src/indexes.rs) keep using `syntrix_core::entity_meta` etc.
pub use syntrix_network::schema;

pub use addr::{parse_device_addr, build_device_addr_string};
pub use heartbeat::start_heartbeat_with_resync;
pub use sync::{PeerStatus, SyncInfo, get_sync_info};
pub use registry::{NamespaceRegistry, Device, RoleGrants, can_access};
pub use schema::{ColumnMeta, ColumnType, ChildTableMeta, EntityMeta, entity_meta, entity_table_name, all_entity_tables, business_columns, child_business_columns, searchable_columns};

pub type OrgId = String;
pub type NodeId = [u8; 32];

pub const ENTITY_NAMES: &[&str] = &["customers", "suppliers", "products", "invoices", "orders", "payroll"];

pub fn schema_version_for(_entity: &str) -> u32 { 1 }

pub fn upcast_payload(payload: serde_json::Value, _from_schema: u32, _to_schema: u32) -> serde_json::Value {
    payload
}
