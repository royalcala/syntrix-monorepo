//! `syntrix-core` — shared P2P sync primitives for `syntrix-admin` and `syntrix-client`.
//!
//! This crate contains logic that was previously duplicated across both Tauri applications.
//! By centralizing it here, any fix or improvement propagates to both apps automatically.

pub mod addr;
pub mod heartbeat;
pub mod sync;

pub use addr::{parse_device_addr, build_device_addr_string};
pub use heartbeat::{start_heartbeat, start_heartbeat_with_resync};
pub use sync::{PeerStatus, SyncInfo, get_sync_info};
