//! # iroh-syntrix-docs
//!
//! Authorization wrapper for [`iroh_docs`] — v0.2.0.
//!
//! Device-centric, multi-org, capability-aware.
//!
//! ## Modules
//!
//! - **NamespaceRegistry:** reads `org_<id>/control` to answer: is this device active?
//!   what namespaces should it open? can it write to X? which org owns namespace Y?
//! - **accept_cb:** consults `NamespaceRegistry` to check if the connecting peer is
//!   active in the org that owns the namespace being synced.
//! - **CapabilityManager:** encrypts/decrypts namespace capabilities per device and role.

pub mod registry;
pub mod accept;
pub mod capabilities;

/// Identifies an organization (e.g. "acme", "clientex").
pub type OrgId = String;

/// A node (device) identifier — Ed25519 public key bytes.
pub type NodeId = [u8; 32];

/// Result type for this crate.
pub type Result<T> = anyhow::Result<T>;
