use crate::repository::{
    ContentId, EdgeFact, NodeFact, NodeKey, NodeKind, RelationKind, SourceSpan,
};

use super::format::{node_kind_from_code, relation_from_code};
use super::{
    CompactEdgeRecord, CompactNodeRecord, RepositoryStoreReadError, StoreFormatError,
    StringTableView,
};

#[derive(Clone, Copy)]
pub struct NodeRecordView<'a> {
    pub node_id: u32,
    record: CompactNodeRecord,
    strings: StringTableView<'a>,
}

impl<'a> NodeRecordView<'a> {
    pub(crate) fn new(
        node_id: u32,
        record: CompactNodeRecord,
        strings: StringTableView<'a>,
    ) -> Self {
        Self {
            node_id,
            record,
            strings,
        }
    }

    pub const fn key(self) -> NodeKey {
        self.record.key
    }

    pub fn kind(self) -> Result<NodeKind, RepositoryStoreReadError> {
        node_kind_from_code(self.record.kind_code)
            .ok_or(StoreFormatError::InvalidNodeKind(self.record.kind_code).into())
    }

    pub fn path(self) -> Result<&'a str, RepositoryStoreReadError> {
        self.strings.get(self.record.path)
    }

    pub fn name(self) -> Result<&'a str, RepositoryStoreReadError> {
        self.strings.get(self.record.name)
    }

    pub fn qualified_name(self) -> Result<&'a str, RepositoryStoreReadError> {
        self.strings.get(self.record.qualified_name)
    }

    pub const fn content_id(self) -> Option<ContentId> {
        self.record.content_id
    }

    pub const fn occurrence_count(self) -> u32 {
        self.record.occurrence_count
    }

    pub fn span(self) -> Result<Option<SourceSpanView<'a>>, RepositoryStoreReadError> {
        self.record
            .span
            .map(|span| SourceSpanView::from_compact(span, self.strings))
            .transpose()
    }

    pub const fn external_identity_digest(self) -> Option<[u8; 32]> {
        match self.record.external_identity {
            Some(identity) => Some(identity.0),
            None => None,
        }
    }

    pub fn external_identity(self) -> Option<String> {
        self.record
            .external_identity
            .map(|identity| identity.canonical_string())
    }

    pub fn materialize(self) -> Result<NodeFact, RepositoryStoreReadError> {
        Ok(NodeFact {
            key: self.key(),
            external_identity: self.external_identity(),
            kind: self.kind()?,
            path: self.path()?.to_owned(),
            name: self.name()?.to_owned(),
            qualified_name: self.qualified_name()?.to_owned(),
            content_id: self.content_id(),
            span: self.span()?.map(SourceSpanView::materialize),
        })
    }
}

#[derive(Clone, Copy)]
pub struct EdgeRecordView<'a> {
    record: CompactEdgeRecord,
    strings: StringTableView<'a>,
}

impl<'a> EdgeRecordView<'a> {
    pub(crate) fn new(record: CompactEdgeRecord, strings: StringTableView<'a>) -> Self {
        Self { record, strings }
    }

    pub const fn source(self) -> NodeKey {
        self.record.source
    }

    pub const fn target(self) -> NodeKey {
        self.record.target
    }

    pub fn relation(self) -> Result<RelationKind, RepositoryStoreReadError> {
        relation_from_code(self.record.relation_code)
            .ok_or(StoreFormatError::InvalidRelation(self.record.relation_code).into())
    }

    pub fn span(self) -> Result<Option<SourceSpanView<'a>>, RepositoryStoreReadError> {
        self.record
            .span
            .map(|span| SourceSpanView::from_compact(span, self.strings))
            .transpose()
    }

    pub fn materialize(self) -> Result<EdgeFact, RepositoryStoreReadError> {
        Ok(EdgeFact {
            source: self.source(),
            target: self.target(),
            relation: self.relation()?,
            span: self.span()?.map(SourceSpanView::materialize),
        })
    }
}

#[derive(Clone, Copy)]
pub struct SourceSpanView<'a> {
    pub path: &'a str,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl<'a> SourceSpanView<'a> {
    pub(crate) fn from_compact(
        span: super::CompactSpan,
        strings: StringTableView<'a>,
    ) -> Result<Self, RepositoryStoreReadError> {
        Ok(Self {
            path: strings.get(span.path)?,
            start_line: span.start_line,
            start_column: span.start_column,
            end_line: span.end_line,
            end_column: span.end_column,
        })
    }

    pub(crate) fn materialize(self) -> SourceSpan {
        SourceSpan {
            path: self.path.to_owned(),
            start_line: self.start_line,
            start_column: self.start_column,
            end_line: self.end_line,
            end_column: self.end_column,
        }
    }
}
