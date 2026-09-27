use crate::repository::{EdgeFact, NodeKey};

use super::format::{
    EDGE_FLAG_SPAN, EDGE_FLAGS_OFFSET, EDGE_RECORD_LEN, EDGE_RELATION_OFFSET, EDGE_SOURCE_OFFSET,
    EDGE_SPAN_END_COLUMN_OFFSET, EDGE_SPAN_END_LINE_OFFSET, EDGE_SPAN_PATH_ID_OFFSET,
    EDGE_SPAN_START_COLUMN_OFFSET, EDGE_SPAN_START_LINE_OFFSET, EDGE_TARGET_OFFSET, relation_code,
    relation_from_code,
};
use super::record_io::{get_u16, get_u32, get_u64, put_u16, put_u32, put_u64};
use super::{CompactSpan, CompactStringTable, StoreFormatError, StringId, StringIdLookup};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactEdgeRecord {
    pub source: NodeKey,
    pub target: NodeKey,
    pub relation_code: u16,
    pub span: Option<CompactSpan>,
}

impl CompactEdgeRecord {
    pub(crate) fn from_fact(
        fact: &EdgeFact,
        strings: &(impl StringIdLookup + ?Sized),
    ) -> Result<Self, StoreFormatError> {
        Ok(Self {
            source: fact.source,
            target: fact.target,
            relation_code: relation_code(&fact.relation),
            span: fact
                .span
                .as_ref()
                .map(|span| CompactSpan::from_source(span, strings))
                .transpose()?,
        })
    }

    pub fn to_fact(self, strings: &CompactStringTable) -> Result<EdgeFact, StoreFormatError> {
        Ok(EdgeFact {
            source: self.source,
            target: self.target,
            relation: relation_from_code(self.relation_code)
                .ok_or(StoreFormatError::InvalidRelation(self.relation_code))?,
            span: self.span.map(|span| span.to_source(strings)).transpose()?,
        })
    }

    pub fn encode(self) -> [u8; EDGE_RECORD_LEN as usize] {
        let mut bytes = [0_u8; EDGE_RECORD_LEN as usize];
        put_u64(&mut bytes, EDGE_SOURCE_OFFSET, self.source.as_u64());
        put_u64(&mut bytes, EDGE_TARGET_OFFSET, self.target.as_u64());
        put_u16(&mut bytes, EDGE_RELATION_OFFSET, self.relation_code);
        let mut flags = 0_u16;
        if let Some(span) = self.span {
            flags |= EDGE_FLAG_SPAN;
            put_span(&mut bytes, span);
        } else {
            put_u32(&mut bytes, EDGE_SPAN_PATH_ID_OFFSET, StringId::ABSENT.0);
        }
        put_u16(&mut bytes, EDGE_FLAGS_OFFSET, flags);
        bytes
    }

    pub fn decode(bytes: &[u8; EDGE_RECORD_LEN as usize]) -> Result<Self, StoreFormatError> {
        let flags = get_u16(bytes, EDGE_FLAGS_OFFSET);
        if flags & !EDGE_FLAG_SPAN != 0 {
            return Err(StoreFormatError::InvalidFlags(flags));
        }
        Ok(Self {
            source: NodeKey::from_u64(get_u64(bytes, EDGE_SOURCE_OFFSET)),
            target: NodeKey::from_u64(get_u64(bytes, EDGE_TARGET_OFFSET)),
            relation_code: get_u16(bytes, EDGE_RELATION_OFFSET),
            span: (flags & EDGE_FLAG_SPAN != 0).then(|| read_span(bytes)),
        })
    }
}

fn put_span(bytes: &mut [u8], span: CompactSpan) {
    put_u32(bytes, EDGE_SPAN_PATH_ID_OFFSET, span.path.0);
    put_u32(bytes, EDGE_SPAN_START_LINE_OFFSET, span.start_line);
    put_u32(bytes, EDGE_SPAN_START_COLUMN_OFFSET, span.start_column);
    put_u32(bytes, EDGE_SPAN_END_LINE_OFFSET, span.end_line);
    put_u32(bytes, EDGE_SPAN_END_COLUMN_OFFSET, span.end_column);
}

fn read_span(bytes: &[u8]) -> CompactSpan {
    CompactSpan {
        path: StringId(get_u32(bytes, EDGE_SPAN_PATH_ID_OFFSET)),
        start_line: get_u32(bytes, EDGE_SPAN_START_LINE_OFFSET),
        start_column: get_u32(bytes, EDGE_SPAN_START_COLUMN_OFFSET),
        end_line: get_u32(bytes, EDGE_SPAN_END_LINE_OFFSET),
        end_column: get_u32(bytes, EDGE_SPAN_END_COLUMN_OFFSET),
    }
}
