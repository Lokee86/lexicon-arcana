mod binary;
mod digest;
mod error;
mod io;
mod lock;
mod model;
mod pending;
mod snapshot;
mod store;

pub use binary::{decode_node_facts, decode_object, encode_object};
pub use digest::object_id;
pub use error::StorageError;
pub use lock::StoreLock;
pub use model::{
    FactObject, FileEntry, LanguageEntry, PendingPublication, RecoveryOutcome, SnapshotManifest,
};
pub use snapshot::{snapshot_bytes, snapshot_id};
pub use store::Store;

pub const OBJECT_VERSION: u64 = 1;
pub const SNAPSHOT_VERSION: u64 = 1;
