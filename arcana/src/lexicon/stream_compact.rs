use std::collections::BTreeMap;

use super::LexiconSnapshotError;
use super::binary_v2_stream::{EdgeRef, NodeRef, UnresolvedRef};
use super::identity::LexiconIdentity;
use super::object::{NodeReference, RecordCounts};
use super::stream_compact_convert::{compact_span, node_kind, relation_code, unresolved_reason};
use super::stream_compact_node::{optional_intern, signature_digest};
use crate::repository::{NodeKey, normalize_repository_path};
use crate::repository_store::{
    CompactRepositoryAssembler, CompactRepositoryBuild, Sha256Identity, StagedNodeError,
};

pub(super) type CompatibilityCounts = BTreeMap<String, usize>;

pub(super) struct CompactPass {
    assembler: CompactRepositoryAssembler,
    planned_counts: RecordCounts,
    relation_counts: RecordCounts,
    compatibility: CompatibilityCounts,
}

impl CompactPass {
    pub(super) fn new() -> Self {
        Self {
            assembler: CompactRepositoryAssembler::with_capacity(0, 0, 0),
            planned_counts: RecordCounts::default(),
            relation_counts: RecordCounts::default(),
            compatibility: BTreeMap::new(),
        }
    }

    pub(super) fn reserve_object(
        &mut self,
        counts: RecordCounts,
    ) -> Result<(), LexiconSnapshotError> {
        self.planned_counts = self
            .planned_counts
            .checked_add(counts)
            .ok_or(LexiconSnapshotError::Malformed("record count overflow"))?;
        self.assembler.reserve_nodes_to(self.planned_counts.nodes);
        Ok(())
    }

    pub(super) fn reserve_relation_object(
        &mut self,
        counts: RecordCounts,
    ) -> Result<(), LexiconSnapshotError> {
        self.relation_counts = self
            .relation_counts
            .checked_add(RecordCounts {
                nodes: 0,
                edges: counts.edges,
                unresolved: counts.unresolved,
            })
            .ok_or(LexiconSnapshotError::Malformed("record count overflow"))?;
        if self.relation_counts.edges > self.planned_counts.edges
            || self.relation_counts.unresolved > self.planned_counts.unresolved
        {
            return Err(LexiconSnapshotError::Malformed(
                "relation count exceeds node-pass plan",
            ));
        }
        self.assembler
            .reserve_relations_to(self.relation_counts.edges, self.relation_counts.unresolved);
        Ok(())
    }

    pub(super) fn ingest_node(&mut self, record: NodeRef<'_>) -> Result<(), LexiconSnapshotError> {
        validate_owner(record.owner)?;
        let path = normalized(record.path)?;
        if record.qualified_name.is_empty() {
            return Err(LexiconSnapshotError::Malformed("node qualified name"));
        }
        let key = record.id.node_key();
        let signature_digest = signature_digest(&record);
        let owner = optional_intern(&mut self.assembler, record.owner)?;
        let kind_code = node_kind(record.kind, &mut self.compatibility);
        let path = self.assembler.intern(&path)?;
        let name = self.assembler.intern(record.name)?;
        let qualified_name = self.assembler.intern(record.qualified_name)?;
        let span = compact_span(&mut self.assembler, record.span)?;
        self.assembler.push_node(
            key,
            Sha256Identity(record.id.digest()),
            signature_digest,
            record.content_id.map(LexiconIdentity::content_id),
            owner,
            kind_code,
            path,
            name,
            qualified_name,
            span,
        );
        Ok(())
    }

    pub(super) fn finish_node_pass(&mut self) -> Result<(), LexiconSnapshotError> {
        self.assembler
            .canonicalize_nodes()
            .map_err(node_staging_error)?;
        Ok(())
    }

    pub(super) fn ingest_edge(&mut self, record: EdgeRef<'_>) -> Result<(), LexiconSnapshotError> {
        validate_owner(record.owner)?;
        let source = self.resolve(record.source)?;
        let target = self.resolve(record.target)?;
        let Some(relation_code) = relation_code(record.relation, "edge", &mut self.compatibility)
        else {
            return Ok(());
        };
        let span = compact_span(&mut self.assembler, record.span)?;
        self.assembler
            .push_edge(source, target, relation_code, span);
        Ok(())
    }

    pub(super) fn ingest_unresolved(
        &mut self,
        record: UnresolvedRef<'_>,
    ) -> Result<(), LexiconSnapshotError> {
        validate_owner(record.owner)?;
        let source = self.resolve(record.source)?;
        let Some(relation_code) =
            relation_code(record.relation, "unresolved", &mut self.compatibility)
        else {
            return Ok(());
        };
        let (reason_code, unknown) = unresolved_reason(record.reason, &mut self.compatibility)?;
        let expression = self.assembler.intern(record.expression)?;
        let candidate_namespace = optional_intern(&mut self.assembler, record.candidate_namespace)?;
        let candidate_name = optional_intern(&mut self.assembler, record.candidate_name)?;
        let unknown_reason = unknown
            .then(|| self.assembler.intern(record.reason))
            .transpose()?;
        let span = compact_span(&mut self.assembler, record.span)?;
        self.assembler.push_unresolved(
            source,
            relation_code,
            reason_code,
            expression,
            candidate_namespace,
            candidate_name,
            unknown_reason,
            span,
        );
        Ok(())
    }

    pub(super) fn finish(
        self,
    ) -> Result<(CompactRepositoryBuild, Vec<String>), LexiconSnapshotError> {
        let Self {
            assembler,
            compatibility,
            ..
        } = self;
        let warnings = compatibility
            .into_iter()
            .map(|(message, count)| format!("{message} ({count} record(s))"))
            .collect();
        Ok((assembler.finish()?, warnings))
    }

    fn resolve(&self, reference: NodeReference) -> Result<NodeKey, LexiconSnapshotError> {
        match reference {
            NodeReference::Key(key) => Ok(key),
            NodeReference::Identity(identity) => {
                let key = identity.node_key();
                self.assembler
                    .contains_node_identity(key, Sha256Identity(identity.digest()))
                    .then_some(key)
                    .ok_or(LexiconSnapshotError::Malformed("unknown relationship node"))
            }
        }
    }
}

fn node_staging_error(error: StagedNodeError) -> LexiconSnapshotError {
    match error {
        StagedNodeError::IdentityCollision { .. } => {
            LexiconSnapshotError::Malformed("node identity collision")
        }
        StagedNodeError::ConflictingDefinition { identity } => {
            LexiconSnapshotError::ConflictingNode(identity.canonical_string())
        }
    }
}

fn validate_owner(owner: Option<&str>) -> Result<(), LexiconSnapshotError> {
    if let Some(owner) = owner {
        normalized(owner)?;
    }
    Ok(())
}

fn normalized(path: &str) -> Result<String, LexiconSnapshotError> {
    normalize_repository_path(path).map_err(|_| LexiconSnapshotError::InvalidPath {
        field: "fact",
        path: path.to_owned(),
    })
}

#[cfg(test)]
#[path = "stream_compact_tests.rs"]
mod tests;
