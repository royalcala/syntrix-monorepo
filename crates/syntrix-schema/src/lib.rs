//! `syntrix-schema` — Schema registry for Syntrix ERP entities.
//!
//! This crate is the **single source of truth** for entity schema definitions.
//! It drives:
//! - Secondary index generation in redb (which fields to index, composite indexes)
//! - Full-text search field selection in Tantivy
//! - Upcaster chain for schema migrations
//! - JSON Schema export for frontend code generation (Zod types, FieldRegistry)
//! - Schema visualizer (admin UI, ER graph)
//! - Audit metadata (field-level relationships)
//!
//! Schemas are defined as Rust constants using a builder pattern.
//! No proc-macros needed — just plain struct constructors.

pub mod schema;
pub mod encoded;
pub mod upcast;
pub mod entities;

pub use schema::*;
pub use encoded::encode_value;
pub use upcast::{Upcaster, apply_upcasters};

/// Re-export build_registry for convenience.
pub use entities::build_registry;

/// Permission resolution: checks if a list of allowed entities (or "*") contains the target.
pub fn can_access(allowed: &[String], entity: &str) -> bool {
    schema::can_access(allowed, entity)
}
