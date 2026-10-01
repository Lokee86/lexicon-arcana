use crate::repository::{
    EdgeFact, NodeFact, SourceSpan, UnresolvedReason, UnresolvedReferenceFact,
};

use super::format::{
    UNKNOWN_REASON_CODE, node_kind_from_code, relation_from_code, unresolved_reason_from_code,
};
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactSpan, CompactUnresolvedRecord,
    RepositoryStoreFile, RepositoryStoreReadError, StoreFormatError, StringId,
};

impl RepositoryStoreFile {
    pub(super) fn materialize_node_record(
        &mut self,
        record: CompactNodeRecord,
    ) -> Result<NodeFact, RepositoryStoreReadError> {
        Ok(NodeFact {
            key: record.key,
            external_identity: record
                .external_identity
                .map(|identity| identity.canonical_string()),
            kind: node_kind_from_code(record.kind_code)
                .ok_or(StoreFormatError::InvalidNodeKind(record.kind_code))?,
            path: self.string(record.path)?,
            name: self.string(record.name)?,
            qualified_name: self.string(record.qualified_name)?,
            content_id: record.content_id,
            span: record
                .span
                .map(|span| self.materialize_span(span))
                .transpose()?,
        })
    }

    pub(super) fn materialize_edge_record(
        &mut self,
        record: CompactEdgeRecord,
    ) -> Result<EdgeFact, RepositoryStoreReadError> {
        Ok(EdgeFact {
            source: record.source,
            target: record.target,
            relation: relation_from_code(record.relation_code)
                .ok_or(StoreFormatError::InvalidRelation(record.relation_code))?,
            span: record
                .span
                .map(|span| self.materialize_span(span))
                .transpose()?,
        })
    }

    pub(super) fn materialize_unresolved_record(
        &mut self,
        record: CompactUnresolvedRecord,
    ) -> Result<UnresolvedReferenceFact, RepositoryStoreReadError> {
        let reason = if record.reason_code == UNKNOWN_REASON_CODE {
            let value = self
                .optional_string(record.unknown_reason)?
                .ok_or(StoreFormatError::MissingUnknownReason)?;
            UnresolvedReason::Unknown(value)
        } else {
            if record.unknown_reason != StringId::ABSENT {
                return Err(StoreFormatError::UnexpectedUnknownReason.into());
            }
            unresolved_reason_from_code(record.reason_code).ok_or(
                StoreFormatError::InvalidUnresolvedReason(record.reason_code),
            )?
        };

        Ok(UnresolvedReferenceFact {
            source: record.source,
            relation: relation_from_code(record.relation_code)
                .ok_or(StoreFormatError::InvalidRelation(record.relation_code))?,
            expression: self.string(record.expression)?,
            candidate_namespace: self.optional_string(record.candidate_namespace)?,
            candidate_name: self.optional_string(record.candidate_name)?,
            reason,
            span: record
                .span
                .map(|span| self.materialize_span(span))
                .transpose()?,
        })
    }

    fn optional_string(
        &mut self,
        id: StringId,
    ) -> Result<Option<String>, RepositoryStoreReadError> {
        if id == StringId::ABSENT {
            Ok(None)
        } else {
            self.string(id).map(Some)
        }
    }

    fn materialize_span(
        &mut self,
        span: CompactSpan,
    ) -> Result<SourceSpan, RepositoryStoreReadError> {
        Ok(SourceSpan {
            path: self.string(span.path)?,
            start_line: span.start_line,
            start_column: span.start_column,
            end_line: span.end_line,
            end_column: span.end_column,
        })
    }
}
