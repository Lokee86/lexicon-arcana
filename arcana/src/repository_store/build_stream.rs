use crate::repository::{ContentId, NodeKey};

use super::build::{CompactRepositoryBuild, CompactRepositoryDelta};
use super::build_stream_finish::{finish_stream_build, finish_stream_delta};
use super::build_stream_nodes::{StagedNodeError, canonicalize_nodes};
use super::string_arena::StagedStringArena;
use super::{RepositoryStoreWriteError, Sha256Identity, TempStringId};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct TempSpan {
    pub path: TempStringId,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TempNodeRecord {
    pub key: NodeKey,
    pub external_identity: Sha256Identity,
    pub signature_digest: [u8; 32],
    pub content_id: Option<ContentId>,
    pub owner: Option<TempStringId>,
    pub path: TempStringId,
    pub name: TempStringId,
    pub qualified_name: TempStringId,
    pub span: Option<TempSpan>,
    pub kind_code: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TempEdgeRecord {
    pub source: NodeKey,
    pub target: NodeKey,
    pub relation_code: u16,
    pub span: Option<TempSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TempUnresolvedRecord {
    pub source: NodeKey,
    pub relation_code: u16,
    pub reason_code: u16,
    pub expression: TempStringId,
    pub candidate_namespace: Option<TempStringId>,
    pub candidate_name: Option<TempStringId>,
    pub unknown_reason: Option<TempStringId>,
    pub span: Option<TempSpan>,
}

pub(crate) struct CompactRepositoryAssembler {
    pub(super) strings: StagedStringArena,
    pub(super) nodes: Vec<TempNodeRecord>,
    pub(super) edges: Vec<TempEdgeRecord>,
    pub(super) unresolved: Vec<TempUnresolvedRecord>,
}

impl CompactRepositoryAssembler {
    pub(crate) fn with_capacity(nodes: usize, edges: usize, unresolved: usize) -> Self {
        Self {
            strings: StagedStringArena::default(),
            nodes: Vec::with_capacity(nodes),
            edges: Vec::with_capacity(edges),
            unresolved: Vec::with_capacity(unresolved),
        }
    }

    pub(crate) fn reserve_nodes_to(&mut self, required: usize) {
        reserve_to(&mut self.nodes, required);
    }

    pub(crate) fn reserve_relations_to(&mut self, edges: usize, unresolved: usize) {
        reserve_to(&mut self.edges, edges);
        reserve_to(&mut self.unresolved, unresolved);
    }

    pub(crate) fn canonicalize_nodes(&mut self) -> Result<(), StagedNodeError> {
        canonicalize_nodes(&mut self.nodes)
    }

    pub(crate) fn contains_node_key(&self, key: NodeKey) -> bool {
        self.nodes
            .binary_search_by_key(&key, |node| node.key)
            .is_ok()
    }

    pub(crate) fn contains_node_identity(&self, key: NodeKey, identity: Sha256Identity) -> bool {
        self.nodes
            .binary_search_by_key(&key, |node| node.key)
            .ok()
            .is_some_and(|index| self.nodes[index].external_identity == identity)
    }

    #[cfg(test)]
    pub(crate) fn capacities(&self) -> (usize, usize, usize) {
        (
            self.nodes.capacity(),
            self.edges.capacity(),
            self.unresolved.capacity(),
        )
    }

    pub(crate) fn intern(
        &mut self,
        value: &str,
    ) -> Result<TempStringId, RepositoryStoreWriteError> {
        self.strings.intern(value).map_err(Into::into)
    }

    pub(crate) fn push_node(
        &mut self,
        key: NodeKey,
        external_identity: Sha256Identity,
        signature_digest: [u8; 32],
        content_id: Option<ContentId>,
        owner: Option<TempStringId>,
        kind_code: u16,
        path: TempStringId,
        name: TempStringId,
        qualified_name: TempStringId,
        span: Option<TempSpan>,
    ) {
        self.nodes.push(TempNodeRecord {
            key,
            external_identity,
            signature_digest,
            content_id,
            owner,
            path,
            name,
            qualified_name,
            span,
            kind_code,
        });
    }

    pub(crate) fn push_edge(
        &mut self,
        source: NodeKey,
        target: NodeKey,
        relation_code: u16,
        span: Option<TempSpan>,
    ) {
        self.edges.push(TempEdgeRecord {
            source,
            target,
            relation_code,
            span,
        });
    }

    pub(crate) fn push_unresolved(
        &mut self,
        source: NodeKey,
        relation_code: u16,
        reason_code: u16,
        expression: TempStringId,
        candidate_namespace: Option<TempStringId>,
        candidate_name: Option<TempStringId>,
        unknown_reason: Option<TempStringId>,
        span: Option<TempSpan>,
    ) {
        self.unresolved.push(TempUnresolvedRecord {
            source,
            relation_code,
            reason_code,
            expression,
            candidate_namespace,
            candidate_name,
            unknown_reason,
            span,
        });
    }

    pub(crate) fn finish(self) -> Result<CompactRepositoryBuild, RepositoryStoreWriteError> {
        finish_stream_build(self)
    }

    pub(crate) fn finish_delta(self) -> Result<CompactRepositoryDelta, RepositoryStoreWriteError> {
        finish_stream_delta(self)
    }
}

fn reserve_to<T>(records: &mut Vec<T>, required: usize) {
    if required <= records.capacity() {
        return;
    }

    const MIN_HEADROOM: usize = 32_768;
    let headroom = (required / 32).max(MIN_HEADROOM);
    let target = required.saturating_add(headroom);
    records.reserve_exact(target.saturating_sub(records.len()));
}
