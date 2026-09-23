//! Rust library boundary for Lexicon.
//!
//! The Go implementation pinned by the Rust migration oracle remains the
//! behavioral reference until each migration slice reaches parity.

pub mod facts;
pub mod identity;

pub use facts::{
    EdgeRecord, FactHeader, FactRecord, FactStream, NodeRecord, SourceSpan, UnresolvedRecord,
    ValidationError,
};
pub use identity::{InvalidSha256Id, content_id, node_id, validate_sha256_id};

pub const PROJECT_NAME: &str = "Lexicon";
pub const PROJECT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FACT_SCHEMA_VERSION: u32 = 1;
