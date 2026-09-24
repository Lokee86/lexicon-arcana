//! Rust library boundary for Lexicon.
//!
//! The Go implementation pinned by the Rust migration oracle remains the
//! behavioral reference until each migration slice reaches parity.

pub mod adapters;
pub mod config;
pub mod facts;
pub mod identity;
pub mod interstack;
pub mod languages;
pub mod repository;
pub mod scan;
pub mod scope;
pub mod storage;

pub use adapters::{
    ADAPTER_SCHEMA_VERSION, AdapterCommand, AdapterError, AdapterHost, AdapterRequest,
    NativeAdapter, adapter_fingerprint, adapter_fingerprint_with_versions, command_spec,
};
pub use config::{
    ANALYSIS_CONFIG_ID, CONFIG_VERSION, Config, config_path, find_adapter_root,
    find_adapter_root_from, load_config, normalize_enabled_languages, save_config,
    save_config_with_languages, state_root, update_enabled_languages,
};
pub use facts::{
    EdgeRecord, FactHeader, FactRecord, FactStream, NodeRecord, SourceSpan, UnresolvedRecord,
    ValidationError,
};
pub use identity::{InvalidSha256Id, content_id, node_id, validate_sha256_id};
pub use repository::{
    IGNORE_FILE_NAME, IgnorePolicy, RepositoryError, SourceMirror, StateRepository,
    ignored_directory, prepare_state_directory,
};
pub use scan::{
    AnalysisPlan, Change, ExecutionBudget, ExecutionPlan, LanguageResult, PlanningInput,
    PublicationTransaction, ScanEngine, ScanExecutionError, ScanPlan, ScanReport,
    assemble_manifest, execute_analysis_plans, execution_plan, execution_plan_with_limits,
    logical_shard_count, plan_scan,
};
pub use scope::build_analysis_scope;
pub use storage::{
    Analysis, FactObject, FileEntry, IncrementalScope, LanguageEntry, PendingPublication,
    RecoveryOutcome, SnapshotManifest, SourceFile, StorageError, Store, StoreLock,
    decode_node_facts, decode_object, encode_object, object_id, snapshot_bytes, snapshot_id,
};

pub const PROJECT_NAME: &str = "Lexicon";
pub const PROJECT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FACT_SCHEMA_VERSION: u32 = 1;
