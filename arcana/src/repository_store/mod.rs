//! Canonical binary repository-store contract and compact value codecs.

#[allow(dead_code)]
mod build;
#[allow(dead_code)]
mod build_indexes;
#[allow(dead_code)]
mod build_ownership;
#[allow(dead_code)]
mod build_stream;
#[allow(dead_code)]
mod build_stream_finish;
#[allow(dead_code)]
mod build_stream_nodes;
mod canonical;
mod edge_record;
mod error;
pub mod format;
mod identity;
mod node_record;
mod reader;
mod reader_error;
mod reader_file;
mod reader_file_lookup;
mod reader_file_validation;
mod reader_incremental;
mod reader_materialize;
mod reader_ownership;
mod reader_queries;
mod reader_records;
mod reader_unresolved;
mod reader_validation;
mod record_io;
mod span;
mod string_view;
mod strings;
mod unresolved_record;
mod writer;
mod writer_error;
mod writer_sections;
mod writer_sink;

#[doc(hidden)]
pub use build::CompactRepositoryBuild;
#[allow(unused_imports)]
pub(crate) use build_stream::{CompactRepositoryAssembler, TempSpan, TempStringId};
pub(crate) use build_stream_nodes::StagedNodeError;
pub use edge_record::CompactEdgeRecord;
pub use error::StoreFormatError;
pub use identity::Sha256Identity;
pub use node_record::CompactNodeRecord;
pub use reader::RepositoryStore;
pub use reader_error::RepositoryStoreReadError;
pub use reader_file::RepositoryStoreFile;
pub use reader_ownership::{ContributionKindView, OwnershipContributionView, OwnershipView};
pub use reader_records::{EdgeRecordView, NodeRecordView, SourceSpanView};
pub use reader_unresolved::UnresolvedRecordView;
pub use span::CompactSpan;
pub use string_view::StringTableView;
pub(crate) use strings::StringIdLookup;
pub use strings::{CompactStringTable, StringId, StringTableBuilder};
pub use unresolved_record::CompactUnresolvedRecord;
#[doc(hidden)]
pub use writer::write_repository_store_compact;
pub use writer::{RepositoryStoreWrite, write_repository_store};
pub use writer_error::RepositoryStoreWriteError;

#[cfg(test)]
mod reader_file_bench;
#[cfg(test)]
mod reader_file_tests;
#[cfg(test)]
mod reader_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod writer_test_support;
#[cfg(test)]
mod writer_tests;
