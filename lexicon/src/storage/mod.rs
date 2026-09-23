mod binary;
mod digest;
mod error;
mod model;
mod snapshot;

pub use binary::{decode_node_facts, decode_object, encode_object};
pub use digest::object_id;
pub use error::StorageError;
pub use model::{FactObject, FileEntry, LanguageEntry, SnapshotManifest};
pub use snapshot::{snapshot_bytes, snapshot_id};

pub const OBJECT_VERSION: u64 = 1;
pub const SNAPSHOT_VERSION: u64 = 1;
