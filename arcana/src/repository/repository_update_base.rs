use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::repository_store::{RepositoryStore, RepositoryStoreFile};
use crate::snapshot::GraphSnapshot;

use super::repository_snapshot_validation::{
    checksum, compare, read_verified, repository_identity_from_checksum,
};
use super::{
    NodeFact, NodeKey, RepositoryCompileError, RepositoryFacts, RepositorySnapshotError,
    RepositorySnapshotManifest,
};

#[derive(Clone, Debug)]
pub struct RepositoryUpdateBase {
    root: PathBuf,
    manifest: RepositorySnapshotManifest,
    graph: GraphSnapshot,
}

impl RepositoryUpdateBase {
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
        let graph = GraphSnapshot::open(root.join(&manifest.graph_manifest_file))?;
        compare(
            "graph_snapshot_id",
            manifest.graph_snapshot_id,
            graph.snapshot_id(),
        )?;
        compare(
            "node_count",
            u64::from(manifest.node_count),
            u64::from(graph.node_count()),
        )?;
        compare("edge_count", manifest.edge_count, graph.edge_count())?;
        Ok(Self {
            root,
            manifest,
            graph,
        })
    }

    pub const fn manifest(&self) -> &RepositorySnapshotManifest {
        &self.manifest
    }

    pub fn open_store(&self) -> Result<RepositoryStore, RepositorySnapshotError> {
        let bytes = fs::read(self.root.join(&self.manifest.repository_store_file))?;
        compare(
            "repository_store_checksum",
            self.manifest.repository_store_checksum,
            checksum(&bytes),
        )?;
        Ok(RepositoryStore::from_bytes(bytes.into_boxed_slice())?)
    }

    pub fn open_incremental_store(&self) -> Result<RepositoryStoreFile, RepositorySnapshotError> {
        let store =
            RepositoryStoreFile::open(self.root.join(&self.manifest.repository_store_file))?;
        compare(
            "repository_store_checksum",
            self.manifest.repository_store_checksum,
            store.artifact_checksum(),
        )?;
        Ok(store)
    }

    pub fn load_facts(&self) -> Result<RepositoryFacts, RepositorySnapshotError> {
        let store = self.open_store()?;
        let facts = store.materialize_facts()?;
        validate_fact_binding(&self.manifest, &facts)?;
        Ok(facts)
    }

    pub fn base_graph_path(&self) -> PathBuf {
        self.root.join(&self.graph.manifest().base_file)
    }
}

fn validate_fact_binding(
    manifest: &RepositorySnapshotManifest,
    facts: &RepositoryFacts,
) -> Result<(), RepositorySnapshotError> {
    let nodes = unique_nodes(&facts.nodes)?;
    compare(
        "node_count",
        u64::from(manifest.node_count),
        nodes.len() as u64,
    )?;

    let mut unresolved = facts.unresolved.iter().collect::<Vec<_>>();
    unresolved.sort_unstable();
    unresolved.dedup();
    compare(
        "unresolved_count",
        manifest.unresolved_count,
        unresolved.len() as u64,
    )?;

    let repository_id =
        repository_identity_from_checksum(facts, manifest.repository_store_checksum);
    if repository_id != manifest.repository_id {
        return Err(RepositorySnapshotError::RepositoryIdentityMismatch {
            expected: manifest.repository_id,
            actual: repository_id,
        });
    }
    Ok(())
}

fn unique_nodes(
    nodes: &[NodeFact],
) -> Result<BTreeMap<NodeKey, &NodeFact>, RepositorySnapshotError> {
    let mut unique = BTreeMap::new();
    for node in nodes {
        if let Some(previous) = unique.get(&node.key)
            && *previous != node
        {
            return Err(RepositoryCompileError::DuplicateConflictingNode { key: node.key }.into());
        }
        unique.entry(node.key).or_insert(node);
    }
    Ok(unique)
}
