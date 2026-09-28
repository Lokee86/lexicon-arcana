use std::collections::BTreeMap;

use crate::repository::{ContentId, NodeKey};

use super::build::CompactRepositoryBuild;
use super::build_stream_finish::finish_stream_build;
use super::{RepositoryStoreWriteError, Sha256Identity};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct TempStringId(pub u32);

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
    pub content_id: Option<ContentId>,
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
    pub(super) strings: BTreeMap<String, TempStringId>,
    pub(super) nodes: Vec<TempNodeRecord>,
    pub(super) edges: Vec<TempEdgeRecord>,
    pub(super) unresolved: Vec<TempUnresolvedRecord>,
}

impl CompactRepositoryAssembler {
    pub(crate) fn with_capacity(nodes: usize, edges: usize, unresolved: usize) -> Self {
        Self {
            strings: BTreeMap::new(),
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
        if let Some(id) = self.strings.get(value) {
            return Ok(*id);
        }
        let id = u32::try_from(self.strings.len())
            .map(TempStringId)
            .map_err(|_| super::StoreFormatError::TooManyStrings)?;
        self.strings.insert(value.to_owned(), id);
        Ok(id)
    }

    pub(crate) fn push_node(
        &mut self,
        key: NodeKey,
        external_identity: Sha256Identity,
        content_id: Option<ContentId>,
        kind_code: u16,
        path: TempStringId,
        name: TempStringId,
        qualified_name: TempStringId,
        span: Option<TempSpan>,
    ) {
        self.nodes.push(TempNodeRecord {
            key,
            external_identity,
            content_id,
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
