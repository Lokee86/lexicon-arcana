mod analysis;
mod binary;
mod dependency;
mod digest;
mod error;
mod export;
mod export_language;
mod gc;
mod gc_execute;
mod gc_storage;
mod gc_validate;
mod io;
mod lock;
mod manifest;
mod materialize;
mod materialize_support;
mod model;
mod pending;
mod snapshot;
mod store;
mod topology;

pub use analysis::Analysis;
pub use binary::{decode_node_facts, decode_object, encode_object};
pub use digest::object_id;
pub use error::StorageError;
pub use gc::{GcOptions, GcPlan, GcResult};
pub use lock::StoreLock;
pub use model::{
    FactObject, FileEntry, IncrementalScope, LanguageEntry, PendingPublication, RecoveryOutcome,
    SnapshotManifest, SourceFile,
};
pub use snapshot::{snapshot_bytes, snapshot_id};
pub use store::Store;

pub const OBJECT_VERSION: u64 = 1;
pub const SNAPSHOT_VERSION: u64 = 1;
