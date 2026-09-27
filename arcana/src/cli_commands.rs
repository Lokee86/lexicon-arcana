use arcana::repository::{
    CatalogueError, FactFileError, IncrementalError, RepositoryCompileError, RepositoryFacts,
    RepositorySnapshotError,
};
use arcana::repository_store::RepositoryStoreWriteError;
use arcana::snapshot::{OverlayError, SnapshotError};
use arcana::storage::{PackedError, QueryError};
use arcana::synthetic::NodeId;
use std::fmt;
use std::fs;
use std::io;

use crate::cli::ImportFactsCommand;
use crate::cli_compile::write_compiled_owned;

#[derive(Debug)]
pub enum CliCommandError {
    Io(io::Error),
    Facts(FactFileError),
    Compile(RepositoryCompileError),
    Packed(PackedError),
    Query(QueryError),
    Catalogue(CatalogueError),
    RepositorySnapshot(RepositorySnapshotError),
    RepositoryStoreWrite(RepositoryStoreWriteError),
    Incremental(IncrementalError),
    Overlay(OverlayError),
    Snapshot(SnapshotError),
    UnknownEdgeKind(u16),
    MissingCatalogueNode(NodeId),
}

impl fmt::Display for CliCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Facts(error) => error.fmt(formatter),
            Self::Compile(error) => error.fmt(formatter),
            Self::Packed(error) => error.fmt(formatter),
            Self::Query(error) => error.fmt(formatter),
            Self::Catalogue(error) => error.fmt(formatter),
            Self::RepositorySnapshot(error) => error.fmt(formatter),
            Self::RepositoryStoreWrite(error) => error.fmt(formatter),
            Self::Incremental(error) => error.fmt(formatter),
            Self::Overlay(error) => error.fmt(formatter),
            Self::Snapshot(error) => error.fmt(formatter),
            Self::UnknownEdgeKind(kind) => {
                write!(formatter, "graph contains unknown edge kind {kind}")
            }
            Self::MissingCatalogueNode(node) => write!(
                formatter,
                "catalogue has no metadata for graph node {}",
                node.0
            ),
        }
    }
}

impl std::error::Error for CliCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Facts(error) => Some(error),
            Self::Compile(error) => Some(error),
            Self::Packed(error) => Some(error),
            Self::Query(error) => Some(error),
            Self::Catalogue(error) => Some(error),
            Self::RepositorySnapshot(error) => Some(error),
            Self::RepositoryStoreWrite(error) => Some(error),
            Self::Incremental(error) => Some(error),
            Self::Overlay(error) => Some(error),
            Self::Snapshot(error) => Some(error),
            Self::UnknownEdgeKind(_) | Self::MissingCatalogueNode(_) => None,
        }
    }
}

impl From<io::Error> for CliCommandError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<FactFileError> for CliCommandError {
    fn from(error: FactFileError) -> Self {
        Self::Facts(error)
    }
}
impl From<RepositoryCompileError> for CliCommandError {
    fn from(error: RepositoryCompileError) -> Self {
        Self::Compile(error)
    }
}
impl From<PackedError> for CliCommandError {
    fn from(error: PackedError) -> Self {
        Self::Packed(error)
    }
}
impl From<QueryError> for CliCommandError {
    fn from(error: QueryError) -> Self {
        Self::Query(error)
    }
}
impl From<CatalogueError> for CliCommandError {
    fn from(error: CatalogueError) -> Self {
        Self::Catalogue(error)
    }
}
impl From<RepositorySnapshotError> for CliCommandError {
    fn from(error: RepositorySnapshotError) -> Self {
        Self::RepositorySnapshot(error)
    }
}
impl From<RepositoryStoreWriteError> for CliCommandError {
    fn from(error: RepositoryStoreWriteError) -> Self {
        Self::RepositoryStoreWrite(error)
    }
}
impl From<IncrementalError> for CliCommandError {
    fn from(error: IncrementalError) -> Self {
        Self::Incremental(error)
    }
}
impl From<OverlayError> for CliCommandError {
    fn from(error: OverlayError) -> Self {
        Self::Overlay(error)
    }
}
impl From<SnapshotError> for CliCommandError {
    fn from(error: SnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

pub fn run_import_facts(command: &ImportFactsCommand) -> Result<String, CliCommandError> {
    if command.output.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "output directory already exists: {}",
                command.output.display()
            ),
        )
        .into());
    }
    let text = fs::read_to_string(&command.facts)?;
    let facts = RepositoryFacts::parse(&text)?;
    fs::create_dir(&command.output)?;
    write_compiled_owned(
        &command.output,
        facts,
        &command.adapter_name,
        &command.adapter_version,
    )
}
