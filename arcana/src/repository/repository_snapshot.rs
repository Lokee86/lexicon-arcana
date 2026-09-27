use std::fs;
use std::path::{Path, PathBuf};

use crate::repository_store::RepositoryStore;
use crate::snapshot::GraphSnapshot;
use crate::storage::QueryError;
use crate::synthetic::GraphDataset;

use super::repository_snapshot_validation::{
    compare, read_verified, repository_identity_from_checksum, validate_compiled_components,
};
use super::{
    RepositoryCatalogue, RepositoryFacts, RepositorySnapshotError, RepositorySnapshotManifest,
    compile_repository_facts, repository_artifact_file_checksum,
};

pub const REPOSITORY_MANIFEST_FILE: &str = "repository.manifest";

#[derive(Clone, Debug)]
pub struct RepositorySnapshot {
    root: PathBuf,
    manifest: RepositorySnapshotManifest,
    graph: GraphSnapshot,
    catalogue: RepositoryCatalogue,
    facts: RepositoryFacts,
    unresolved: RepositoryFacts,
}

impl RepositorySnapshot {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepositorySnapshotError> {
        let path = path.as_ref();
        let root = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let manifest = RepositorySnapshotManifest::decode(&fs::read_to_string(path)?)?;
        read_verified(
            &root,
            &manifest.graph_manifest_file,
            manifest.graph_manifest_checksum,
            "graph_manifest_checksum",
        )?;
        let store_path = root.join(&manifest.repository_store_file);
        compare(
            "repository_store_checksum",
            manifest.repository_store_checksum,
            repository_artifact_file_checksum(&store_path)?,
        )?;

        let graph = GraphSnapshot::open(root.join(&manifest.graph_manifest_file))?;
        let store = RepositoryStore::open(&store_path)?;
        let facts = store.materialize_facts()?;
        let compiled = compile_repository_facts(&facts)?;
        let repository_id =
            repository_identity_from_checksum(&facts, manifest.repository_store_checksum);
        validate_compiled_components(&manifest, &graph, &compiled, repository_id)?;

        let unresolved =
            RepositoryFacts::with_unresolved(Vec::new(), Vec::new(), compiled.unresolved.clone());
        Ok(Self {
            root,
            manifest,
            graph,
            catalogue: compiled.catalogue,
            facts,
            unresolved,
        })
    }

    pub const fn manifest(&self) -> &RepositorySnapshotManifest {
        &self.manifest
    }

    pub const fn graph(&self) -> &GraphSnapshot {
        &self.graph
    }

    pub const fn catalogue(&self) -> &RepositoryCatalogue {
        &self.catalogue
    }

    pub const fn facts(&self) -> &RepositoryFacts {
        &self.facts
    }

    pub const fn unresolved(&self) -> &RepositoryFacts {
        &self.unresolved
    }

    pub fn into_protocol_parts(self) -> (GraphSnapshot, RepositoryCatalogue, RepositoryFacts) {
        (self.graph, self.catalogue, self.unresolved)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn base_graph_path(&self) -> PathBuf {
        self.root.join(&self.graph.manifest().base_file)
    }

    pub fn materialize_base_dataset(&self) -> Result<GraphDataset, QueryError> {
        self.graph.materialize_base_dataset()
    }
}
