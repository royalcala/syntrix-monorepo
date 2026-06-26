//! `syntrix-core` — shared P2P sync primitives for `syntrix-admin` and `syntrix-client`.
//!
//! This crate contains logic that was previously duplicated across both Tauri applications.
//! By centralizing it here, any fix or improvement propagates to both apps automatically.

pub mod addr;
pub mod heartbeat;
pub mod sync;
pub mod registry;
pub mod accept;
pub mod capabilities;

pub use addr::{parse_device_addr, build_device_addr_string};
pub use heartbeat::{start_heartbeat, start_heartbeat_with_resync};
pub use sync::{PeerStatus, SyncInfo, get_sync_info};
pub use registry::{NamespaceRegistry, Device, RoleGrants};
pub use accept::make_accept_cb;
pub use capabilities::CapabilityManager;

/// Identifies an organization (e.g. "acme", "clientex").
pub type OrgId = String;

/// A node (device) identifier — Ed25519 public key bytes.
pub type NodeId = [u8; 32];
