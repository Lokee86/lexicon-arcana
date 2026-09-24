//! Rust library boundary for Lexicon.
//!
//! The Go implementation pinned by the Rust migration oracle remains the
//! behavioral reference until each migration slice reaches parity.

pub mod facts;
pub mod identity;
pub mod languages;
pub mod scan;
pub mod storage;

pub use facts::{
    EdgeRecord, FactHeader, FactRecord, FactStream, NodeRecord, SourceSpan, UnresolvedRecord,
    ValidationError,
};
pub use identity::{InvalidSha256Id, content_id, node_id, validate_sha256_id};
pub use scan::{
    AnalysisPlan, Change, LanguageResult, PlanningInput, PublicationTransaction, ScanPlan,
    assemble_manifest, plan_scan,
};
pub use storage::{
    Analysis, FactObject, FileEntry, IncrementalScope, LanguageEntry, PendingPublication,
    RecoveryOutcome, SnapshotManifest, SourceFile, StorageError, Store, StoreLock,
    decode_node_facts, decode_object, encode_object, object_id, snapshot_bytes, snapshot_id,
};

pub const PROJECT_NAME: &str = "Lexicon";
pub const PROJECT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FACT_SCHEMA_VERSION: u32 = 1;
