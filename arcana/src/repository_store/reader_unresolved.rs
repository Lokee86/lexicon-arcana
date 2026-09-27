use crate::repository::{NodeKey, RelationKind, UnresolvedReason, UnresolvedReferenceFact};

use super::format::{UNKNOWN_REASON_CODE, relation_from_code, unresolved_reason_from_code};
use super::{
    CompactUnresolvedRecord, RepositoryStoreReadError, SourceSpanView, StoreFormatError,
    StringTableView,
};

#[derive(Clone, Copy)]
pub struct UnresolvedRecordView<'a> {
    record: CompactUnresolvedRecord,
    strings: StringTableView<'a>,
}

impl<'a> UnresolvedRecordView<'a> {
    pub(crate) fn new(record: CompactUnresolvedRecord, strings: StringTableView<'a>) -> Self {
        Self { record, strings }
    }

    pub const fn source(self) -> NodeKey {
        self.record.source
    }

    pub fn relation(self) -> Result<RelationKind, RepositoryStoreReadError> {
        relation_from_code(self.record.relation_code)
            .ok_or(StoreFormatError::InvalidRelation(self.record.relation_code).into())
    }

    pub fn expression(self) -> Result<&'a str, RepositoryStoreReadError> {
        self.strings.get(self.record.expression)
    }

    pub fn candidate_namespace(self) -> Result<Option<&'a str>, RepositoryStoreReadError> {
        self.strings.optional(self.record.candidate_namespace)
    }

    pub fn candidate_name(self) -> Result<Option<&'a str>, RepositoryStoreReadError> {
        self.strings.optional(self.record.candidate_name)
    }

    pub fn reason(self) -> Result<UnresolvedReason, RepositoryStoreReadError> {
        if self.record.reason_code == UNKNOWN_REASON_CODE {
            let value = self
                .strings
                .optional(self.record.unknown_reason)?
                .ok_or(StoreFormatError::MissingUnknownReason)?;
            Ok(UnresolvedReason::Unknown(value.to_owned()))
        } else {
            unresolved_reason_from_code(self.record.reason_code)
                .ok_or(StoreFormatError::InvalidUnresolvedReason(self.record.reason_code).into())
        }
    }

    pub fn span(self) -> Result<Option<SourceSpanView<'a>>, RepositoryStoreReadError> {
        self.record
            .span
            .map(|span| SourceSpanView::from_compact(span, self.strings))
            .transpose()
    }

    pub fn materialize(self) -> Result<UnresolvedReferenceFact, RepositoryStoreReadError> {
        Ok(UnresolvedReferenceFact {
            source: self.source(),
            relation: self.relation()?,
            expression: self.expression()?.to_owned(),
            candidate_namespace: self.candidate_namespace()?.map(str::to_owned),
            candidate_name: self.candidate_name()?.map(str::to_owned),
            reason: self.reason()?,
            span: self.span()?.map(SourceSpanView::materialize),
        })
    }
}
