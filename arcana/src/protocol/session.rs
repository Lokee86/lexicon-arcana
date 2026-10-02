use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::repository::{CatalogueEntry, REPOSITORY_MANIFEST_FILE, RepositoryQuerySnapshot};
use crate::synthetic::NodeId;

use super::error::ProtocolError;
use super::request::{RequestCommand, RequestEnvelope};
use super::response::{failure, success};

/// One opened repository snapshot serving repeated protocol queries.
#[derive(Debug)]
pub struct ProtocolSnapshot {
    pub(crate) root: PathBuf,
    pub(crate) query: RepositoryQuerySnapshot,
}

impl ProtocolSnapshot {
    /// Opens and validates a manifest-bound repository snapshot.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ProtocolError> {
        let root = root.as_ref().to_path_buf();
        let query = RepositoryQuerySnapshot::open(root.join(REPOSITORY_MANIFEST_FILE))
            .map_err(|error| ProtocolError::InvalidSnapshot(error.to_string()))?;
        Ok(Self { root, query })
    }

    /// Handles one JSON request and always returns one JSON response.
    pub fn handle_line(&self, line: &str) -> Value {
        let request = match serde_json::from_str::<RequestEnvelope>(line) {
            Ok(request) => request,
            Err(error) => return failure(Value::Null, "invalid_json", error.to_string()),
        };
        let id = request.id;
        match self.execute(request.command) {
            Ok(result) => success(id, result),
            Err(error) => failure(id, error.code, error.message),
        }
    }

    fn execute(&self, command: RequestCommand) -> Result<Value, RequestFailure> {
        match command {
            RequestCommand::Capabilities => Ok(serde_json::json!({
                "protocol": "arcana.query.v1",
                "version": 1,
                "implementation_version": env!("CARGO_PKG_VERSION"),
                "operations": [
                    "capabilities",
                    "search_nodes",
                    "resolve_symbol",
                    "resolve_file",
                    "list_nodes",
                    "export_graph",
                    "neighbors",
                    "paths",
                    "reachability",
                    "impact",
                    "shortest_call_chain",
                    "dead_symbols",
                    "operational_role",
                    "architecture_summary",
                    "unresolved",
                    "stats",
                    "diff"
                ]
            })),
            RequestCommand::SearchNodes { query, limit } => self.search_nodes(&query, limit),
            RequestCommand::ResolveSymbol {
                name,
                kind,
                path,
                limit,
            } => self.resolve_symbol(&name, kind.as_deref(), path.as_deref(), limit),
            RequestCommand::ResolveFile { path, limit } => self.resolve_file(&path, limit),
            RequestCommand::ListNodes {
                kind,
                path_prefix,
                offset,
                limit,
            } => self.list_nodes(kind.as_deref(), path_prefix.as_deref(), offset, limit),
            RequestCommand::ExportGraph {
                path_prefix,
                offset,
                limit,
                pinned_node_ids,
            } => self.export_graph(
                path_prefix.as_deref(),
                offset,
                limit,
                pinned_node_ids.as_deref().unwrap_or(&[]),
            ),
            RequestCommand::Neighbors {
                node_id,
                direction,
                relation,
                relations,
                limit,
            } => self.neighbors(
                node_id,
                direction,
                relation.as_deref(),
                relations.as_deref(),
                limit,
            ),
            RequestCommand::Paths {
                from_node_id,
                to_node_id,
                relations,
                max_depth,
                limit,
            } => self.paths(
                from_node_id,
                to_node_id,
                relations.as_deref(),
                max_depth,
                limit,
            ),
            RequestCommand::Reachability {
                entry_node_ids,
                include_possible,
                max_depth,
                limit,
            } => self.reachability(
                &entry_node_ids,
                include_possible.unwrap_or(true),
                max_depth,
                limit,
            ),
            RequestCommand::Impact {
                node_id,
                relations,
                max_depth,
                limit,
            } => self.impact(node_id, relations.as_deref(), max_depth, limit),
            RequestCommand::ShortestCallChain {
                from_node_id,
                to_node_id,
                include_possible,
                max_depth,
            } => self.shortest_call_chain(
                from_node_id,
                to_node_id,
                include_possible.unwrap_or(true),
                max_depth,
            ),
            RequestCommand::DeadSymbols {
                entry_node_ids,
                include_possible,
                kinds,
                max_depth,
                limit,
            } => self.dead_symbols(
                &entry_node_ids,
                include_possible.unwrap_or(true),
                kinds.as_deref(),
                max_depth,
                limit,
            ),
            RequestCommand::OperationalRole {
                node_id,
                entry_node_ids,
                include_possible,
                max_depth,
            } => self.operational_role(
                node_id,
                entry_node_ids.as_deref(),
                include_possible.unwrap_or(true),
                max_depth,
            ),
            RequestCommand::ArchitectureSummary {
                path_prefix,
                relations,
                min_community_size,
                limit,
            } => self.architecture_summary(
                path_prefix.as_deref(),
                relations.as_deref(),
                min_community_size,
                limit,
            ),
            RequestCommand::Unresolved {
                node_id,
                path,
                reason,
                relation,
                limit,
            } => self.query_unresolved(
                node_id,
                path.as_deref(),
                reason.as_deref(),
                relation.as_deref(),
                limit,
            ),
            RequestCommand::Stats => self.stats(),
            RequestCommand::Diff {
                other_snapshot,
                limit,
            } => self.diff_snapshot(&other_snapshot, limit),
        }
    }

    pub(crate) fn entry(&self, node_id: NodeId) -> Result<Option<CatalogueEntry>, RequestFailure> {
        Ok(self.query.entry(node_id)?)
    }
}

#[derive(Debug)]
pub(crate) struct RequestFailure {
    pub code: &'static str,
    pub message: String,
}
impl From<crate::repository_store::RepositoryStoreReadError> for RequestFailure {
    fn from(error: crate::repository_store::RepositoryStoreReadError) -> Self {
        Self::new("invalid_snapshot", error.to_string())
    }
}
impl RequestFailure {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
