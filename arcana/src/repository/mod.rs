//! Language-neutral repository facts and deterministic compatibility formats.

mod catalogue;
mod compiler;
#[cfg(test)]
mod compiler_catalogue_tests;
mod fact_encode;
mod fact_file;
mod fact_file_error;
#[cfg(test)]
mod fact_file_tests;
mod graph_compile;
mod incremental;
mod incremental_diff;
mod incremental_store;
#[cfg(test)]
mod incremental_store_tests;
#[cfg(test)]
mod incremental_tests;
#[cfg(test)]
mod lexicon_fact_file_tests;
mod model;
mod ownership;
#[cfg(test)]
mod ownership_tests;
mod path;
mod relation_codes;
mod repository_publish;
mod repository_publish_graph;
#[cfg(test)]
mod repository_publish_graph_tests;
mod repository_snapshot;
mod repository_snapshot_error;
mod repository_snapshot_format;
mod repository_snapshot_format_support;
#[cfg(test)]
mod repository_snapshot_test_support;
#[cfg(test)]
mod repository_snapshot_tests;
mod repository_snapshot_validation;
mod repository_update_base;
mod unresolved;

pub use catalogue::{
    CatalogueEntry, CatalogueError, RepositoryCatalogue, read_catalogue, write_catalogue,
};
pub use compiler::{
    CompiledRepository, RepositoryCompileError, compile_facts, compile_repository_facts,
    compile_repository_facts_owned,
};
pub use fact_encode::{encode_facts, encode_unresolved_facts};
pub use fact_file::{FACT_SCHEMA_VERSION, parse_facts};
pub use fact_file_error::FactFileError;
#[allow(unused_imports)]
pub(crate) use graph_compile::compile_compact_repository_graph;
pub use graph_compile::{CompiledRepositoryGraph, compile_repository_graph};
pub use incremental::{
    IncrementalError, IncrementalUpdate, plan_file_update, plan_file_update_from_verified_base,
};
pub use incremental_store::{VerifiedSnapshotUpdatePlan, plan_verified_snapshot_update_from_store};
pub use model::{ContentId, EdgeFact, NodeFact, NodeKey, NodeKind, RelationKind, SourceSpan};
pub use ownership::{
    FactOwnershipError, FactPartitions, partition_facts, replace_changed_files,
    replace_changed_files_owned_base,
};
pub(crate) use ownership::{collect_node_owners, edge_owner, node_owner, unresolved_owner};
pub use path::{RepositoryPathError, normalize_repository_path};
pub use relation_codes::{edge_kind_to_relation, relation_to_edge_kind};
pub use repository_publish::{
    PublishRepositorySnapshot, RepositoryArtifactChecksums, derive_repository_snapshot_id,
    publish_precompiled_repository_snapshot, publish_precompiled_repository_snapshot_with_identity,
    publish_repository_snapshot, repository_artifact_checksum, repository_artifact_file_checksum,
    repository_identity_for_facts,
};
pub use repository_publish_graph::publish_graph_repository_snapshot_with_identity;
pub use repository_snapshot::{REPOSITORY_MANIFEST_FILE, RepositorySnapshot};
pub use repository_snapshot_error::RepositorySnapshotError;
pub use repository_snapshot_format::{REPOSITORY_MANIFEST_VERSION, RepositorySnapshotManifest};
pub use repository_update_base::RepositoryUpdateBase;
pub use unresolved::{UnresolvedReason, UnresolvedReferenceFact};

/// The complete set of facts extracted from one repository.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepositoryFacts {
    pub nodes: Vec<NodeFact>,
    pub edges: Vec<EdgeFact>,
    pub unresolved: Vec<UnresolvedReferenceFact>,
}

impl RepositoryFacts {
    pub fn new(nodes: Vec<NodeFact>, edges: Vec<EdgeFact>) -> Self {
        Self {
            nodes,
            edges,
            unresolved: Vec::new(),
        }
    }

    pub fn with_unresolved(
        nodes: Vec<NodeFact>,
        edges: Vec<EdgeFact>,
        unresolved: Vec<UnresolvedReferenceFact>,
    ) -> Self {
        Self {
            nodes,
            edges,
            unresolved,
        }
    }

    /// Returns the canonical tab-separated representation of these facts.
    pub fn encode(&self) -> String {
        encode_facts(self)
    }

    /// Parses a canonical tab-separated fact file.
    pub fn parse(input: &str) -> Result<Self, FactFileError> {
        parse_facts(input)
    }

    /// Produces a copy with nodes and edges in canonical order.
    pub fn canonicalized(&self) -> Self {
        let mut facts = self.clone();
        facts.nodes.sort_unstable();
        facts.edges.sort_unstable();
        facts.unresolved.sort_unstable();
        facts
    }
}
