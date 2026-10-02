//! Immutable query generation: validated graph plus file-backed metadata.
use super::repository_snapshot_validation::{compare, read_verified};
use super::{
    CatalogueEntry, NodeKey, NodeKind, RepositorySnapshotError, RepositorySnapshotManifest,
};
use crate::repository_store::{RepositoryStoreFile, RepositoryStoreReadError};
use crate::snapshot::GraphSnapshot;
use crate::synthetic::NodeId;
use std::cell::RefCell;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct RepositoryQuerySnapshot {
    graph: GraphSnapshot,
    manifest: RepositorySnapshotManifest,
    store: RefCell<RepositoryStoreFile>,
}
impl RepositoryQuerySnapshot {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepositorySnapshotError> {
        let trace = std::env::var_os("ARCANA_QUERY_TRACE").is_some();
        let start = std::time::Instant::now();
        let path = path.as_ref();
        let root = path.parent().unwrap_or_else(|| Path::new("."));
        let manifest = RepositorySnapshotManifest::decode(&fs::read_to_string(path)?)?;
        read_verified(
            root,
            &manifest.graph_manifest_file,
            manifest.graph_manifest_checksum,
            "graph_manifest_checksum",
        )?;
        if trace {
            eprintln!(
                "arcana query manifest_validation_ms={:.3}",
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
        let graph_start = std::time::Instant::now();
        let graph = GraphSnapshot::open(root.join(&manifest.graph_manifest_file))?;
        if trace {
            eprintln!(
                "arcana query graph_open_ms={:.3}",
                graph_start.elapsed().as_secs_f64() * 1000.0
            );
        }
        let store_start = std::time::Instant::now();
        let mut store = RepositoryStoreFile::open(root.join(&manifest.repository_store_file))?;
        if trace {
            eprintln!(
                "arcana query store_validation_ms={:.3}",
                store_start.elapsed().as_secs_f64() * 1000.0
            );
        }
        compare(
            "repository_store_checksum",
            manifest.repository_store_checksum,
            store.artifact_checksum(),
        )?;
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
        compare(
            "catalogue_length",
            u64::from(store.node_count()),
            u64::from(graph.node_count()),
        )?;
        compare("edge_count", manifest.edge_count, graph.edge_count())?;
        compare(
            "unresolved_count",
            manifest.unresolved_count,
            store.unique_unresolved_count()?,
        )?;
        let ids = store.node_ids_by_kind(&NodeKind::Repository)?;
        // Duplicate occurrence facts deliberately affect the legacy identity rule.
        let identity = if ids.len() == 1 && store.node_occurrences(ids[0])? == 1 {
            store.entry(ids[0])?.expect("indexed node").fact.key.0
        } else {
            store.artifact_checksum()
        };
        compare("repository_id", manifest.repository_id, identity)?;
        compare(
            "snapshot_id",
            manifest.snapshot_id,
            super::derive_repository_snapshot_id(
                manifest.repository_id,
                graph.snapshot_id(),
                manifest.repository_store_checksum,
                &manifest.adapter_name,
                &manifest.adapter_version,
            ),
        )?;
        if trace {
            eprintln!(
                "arcana query startup_ms={:.3} fact_materializations=0 compiler_invocations=0",
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
        Ok(Self {
            graph,
            manifest,
            store: RefCell::new(store),
        })
    }
    pub fn graph(&self) -> &GraphSnapshot {
        &self.graph
    }
    pub fn manifest(&self) -> &RepositorySnapshotManifest {
        &self.manifest
    }
    pub fn node_kind_counts(
        &self,
    ) -> Result<std::collections::BTreeMap<String, u64>, RepositoryStoreReadError> {
        self.store.borrow_mut().node_kind_counts()
    }
    pub fn unresolved_statistics(
        &self,
    ) -> Result<(std::collections::BTreeMap<String, u64>, u64), RepositoryStoreReadError> {
        self.store.borrow_mut().unresolved_statistics()
    }
    pub fn search_matches(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<(usize, Vec<crate::repository_store::SearchMatch>), RepositoryStoreReadError> {
        self.store.borrow_mut().search_matches(query, limit)
    }
    pub fn visit_unresolved(
        &self,
        source: Option<NodeKey>,
        reason: Option<&super::UnresolvedReason>,
        relation: Option<&super::RelationKind>,
        path: Option<&str>,
        visit: impl FnMut(u64, NodeId),
    ) -> Result<(), RepositoryStoreReadError> {
        self.store
            .borrow_mut()
            .visit_unresolved(source, reason, relation, path, visit)
    }
    pub fn entry(&self, id: NodeId) -> Result<Option<CatalogueEntry>, RepositoryStoreReadError> {
        self.store.borrow_mut().entry(id)
    }
    pub fn node_id(&self, key: NodeKey) -> Result<Option<NodeId>, RepositoryStoreReadError> {
        self.store.borrow_mut().node_id(key)
    }
    pub fn node_ids_by_name(&self, value: &str) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        self.store.borrow_mut().node_ids_by_name(value)
    }
    pub fn node_ids_by_path(&self, value: &str) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        self.store.borrow_mut().node_ids_by_path(value)
    }
    pub fn node_ids_by_path_prefix(
        &self,
        value: &str,
    ) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        self.store.borrow_mut().node_ids_by_path_prefix(value)
    }
    pub fn node_ids_by_kind(
        &self,
        kind: &NodeKind,
    ) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        self.store.borrow_mut().node_ids_by_kind(kind)
    }
    pub fn len(&self) -> usize {
        self.manifest.node_count as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn entries(
        &self,
    ) -> impl Iterator<Item = Result<CatalogueEntry, RepositoryStoreReadError>> + '_ {
        (0..self.manifest.node_count).map(|id| {
            self.entry(NodeId(id))
                .and_then(|entry| entry.ok_or(RepositoryStoreReadError::InvalidNodeId(id)))
        })
    }
    pub fn unresolved_range(
        &self,
        key: Option<NodeKey>,
    ) -> Result<std::ops::Range<u64>, RepositoryStoreReadError> {
        match key {
            Some(key) => self.store.borrow_mut().unresolved_range(key),
            None => Ok(0..self.store.borrow().unresolved_count()),
        }
    }
    pub fn unresolved(
        &self,
        index: u64,
    ) -> Result<super::UnresolvedReferenceFact, RepositoryStoreReadError> {
        self.store.borrow_mut().unresolved(index)
    }
}
