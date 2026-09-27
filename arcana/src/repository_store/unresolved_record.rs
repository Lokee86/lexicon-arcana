use crate::repository::{NodeKey, UnresolvedReason, UnresolvedReferenceFact};

use super::format::{
    UNKNOWN_REASON_CODE, UNRESOLVED_CANDIDATE_NAME_ID_OFFSET,
    UNRESOLVED_CANDIDATE_NAMESPACE_ID_OFFSET, UNRESOLVED_EXPRESSION_ID_OFFSET,
    UNRESOLVED_FLAG_SPAN, UNRESOLVED_FLAGS_OFFSET, UNRESOLVED_REASON_OFFSET,
    UNRESOLVED_REASON_STRING_ID_OFFSET, UNRESOLVED_RECORD_LEN, UNRESOLVED_RELATION_OFFSET,
    UNRESOLVED_SOURCE_OFFSET, UNRESOLVED_SPAN_END_COLUMN_OFFSET, UNRESOLVED_SPAN_END_LINE_OFFSET,
    UNRESOLVED_SPAN_PATH_ID_OFFSET, UNRESOLVED_SPAN_START_COLUMN_OFFSET,
    UNRESOLVED_SPAN_START_LINE_OFFSET, relation_code, relation_from_code, unresolved_reason_code,
    unresolved_reason_from_code,
};
use super::record_io::{get_u16, get_u32, get_u64, put_u16, put_u32, put_u64};
use super::{CompactSpan, CompactStringTable, StoreFormatError, StringId, StringIdLookup};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactUnresolvedRecord {
    pub source: NodeKey,
    pub relation_code: u16,
    pub reason_code: u16,
    pub expression: StringId,
    pub candidate_namespace: StringId,
    pub candidate_name: StringId,
    pub unknown_reason: StringId,
    pub span: Option<CompactSpan>,
}

impl CompactUnresolvedRecord {
    pub(crate) fn from_fact(
        fact: &UnresolvedReferenceFact,
        strings: &(impl StringIdLookup + ?Sized),
    ) -> Result<Self, StoreFormatError> {
        let unknown_reason = match &fact.reason {
            UnresolvedReason::Unknown(value) => strings.id(value)?,
            _ => StringId::ABSENT,
        };
        Ok(Self {
            source: fact.source,
            relation_code: relation_code(&fact.relation),
            reason_code: unresolved_reason_code(&fact.reason),
            expression: strings.id(&fact.expression)?,
            candidate_namespace: optional_id(strings, fact.candidate_namespace.as_deref())?,
            candidate_name: optional_id(strings, fact.candidate_name.as_deref())?,
            unknown_reason,
            span: fact
                .span
                .as_ref()
                .map(|span| CompactSpan::from_source(span, strings))
                .transpose()?,
        })
    }

    pub fn to_fact(
        self,
        strings: &CompactStringTable,
    ) -> Result<UnresolvedReferenceFact, StoreFormatError> {
        let reason = if self.reason_code == UNKNOWN_REASON_CODE {
            let value = strings
                .optional(self.unknown_reason)?
                .ok_or(StoreFormatError::MissingUnknownReason)?;
            UnresolvedReason::Unknown(value.to_owned())
        } else {
            if self.unknown_reason != StringId::ABSENT {
                return Err(StoreFormatError::UnexpectedUnknownReason);
            }
            unresolved_reason_from_code(self.reason_code)
                .ok_or(StoreFormatError::InvalidUnresolvedReason(self.reason_code))?
        };
        Ok(UnresolvedReferenceFact {
            source: self.source,
            relation: relation_from_code(self.relation_code)
                .ok_or(StoreFormatError::InvalidRelation(self.relation_code))?,
            expression: strings.get(self.expression)?.to_owned(),
            candidate_namespace: optional_string(strings, self.candidate_namespace)?,
            candidate_name: optional_string(strings, self.candidate_name)?,
            reason,
            span: self.span.map(|span| span.to_source(strings)).transpose()?,
        })
    }

    pub fn encode(self) -> [u8; UNRESOLVED_RECORD_LEN as usize] {
        let mut bytes = [0_u8; UNRESOLVED_RECORD_LEN as usize];
        put_u64(&mut bytes, UNRESOLVED_SOURCE_OFFSET, self.source.as_u64());
        put_u16(&mut bytes, UNRESOLVED_RELATION_OFFSET, self.relation_code);
        put_u16(&mut bytes, UNRESOLVED_REASON_OFFSET, self.reason_code);
        let mut flags = 0_u16;
        if let Some(span) = self.span {
            flags |= UNRESOLVED_FLAG_SPAN;
            put_span(&mut bytes, span);
        } else {
            put_u32(
                &mut bytes,
                UNRESOLVED_SPAN_PATH_ID_OFFSET,
                StringId::ABSENT.0,
            );
        }
        put_u16(&mut bytes, UNRESOLVED_FLAGS_OFFSET, flags);
        put_u32(
            &mut bytes,
            UNRESOLVED_EXPRESSION_ID_OFFSET,
            self.expression.0,
        );
        put_u32(
            &mut bytes,
            UNRESOLVED_CANDIDATE_NAMESPACE_ID_OFFSET,
            self.candidate_namespace.0,
        );
        put_u32(
            &mut bytes,
            UNRESOLVED_CANDIDATE_NAME_ID_OFFSET,
            self.candidate_name.0,
        );
        put_u32(
            &mut bytes,
            UNRESOLVED_REASON_STRING_ID_OFFSET,
            self.unknown_reason.0,
        );
        bytes
    }

    pub fn decode(bytes: &[u8; UNRESOLVED_RECORD_LEN as usize]) -> Result<Self, StoreFormatError> {
        let flags = get_u16(bytes, UNRESOLVED_FLAGS_OFFSET);
        if flags & !UNRESOLVED_FLAG_SPAN != 0 {
            return Err(StoreFormatError::InvalidFlags(flags));
        }
        Ok(Self {
            source: NodeKey::from_u64(get_u64(bytes, UNRESOLVED_SOURCE_OFFSET)),
            relation_code: get_u16(bytes, UNRESOLVED_RELATION_OFFSET),
            reason_code: get_u16(bytes, UNRESOLVED_REASON_OFFSET),
            expression: StringId(get_u32(bytes, UNRESOLVED_EXPRESSION_ID_OFFSET)),
            candidate_namespace: StringId(get_u32(bytes, UNRESOLVED_CANDIDATE_NAMESPACE_ID_OFFSET)),
            candidate_name: StringId(get_u32(bytes, UNRESOLVED_CANDIDATE_NAME_ID_OFFSET)),
            unknown_reason: StringId(get_u32(bytes, UNRESOLVED_REASON_STRING_ID_OFFSET)),
            span: (flags & UNRESOLVED_FLAG_SPAN != 0).then(|| read_span(bytes)),
        })
    }
}

fn optional_id(
    strings: &(impl StringIdLookup + ?Sized),
    value: Option<&str>,
) -> Result<StringId, StoreFormatError> {
    value
        .map(|value| strings.id(value))
        .transpose()
        .map(StringId::optional)
}

fn optional_string(
    strings: &CompactStringTable,
    id: StringId,
) -> Result<Option<String>, StoreFormatError> {
    Ok(strings.optional(id)?.map(str::to_owned))
}

fn put_span(bytes: &mut [u8], span: CompactSpan) {
    put_u32(bytes, UNRESOLVED_SPAN_PATH_ID_OFFSET, span.path.0);
    put_u32(bytes, UNRESOLVED_SPAN_START_LINE_OFFSET, span.start_line);
    put_u32(
        bytes,
        UNRESOLVED_SPAN_START_COLUMN_OFFSET,
        span.start_column,
    );
    put_u32(bytes, UNRESOLVED_SPAN_END_LINE_OFFSET, span.end_line);
    put_u32(bytes, UNRESOLVED_SPAN_END_COLUMN_OFFSET, span.end_column);
}

fn read_span(bytes: &[u8]) -> CompactSpan {
    CompactSpan {
        path: StringId(get_u32(bytes, UNRESOLVED_SPAN_PATH_ID_OFFSET)),
        start_line: get_u32(bytes, UNRESOLVED_SPAN_START_LINE_OFFSET),
        start_column: get_u32(bytes, UNRESOLVED_SPAN_START_COLUMN_OFFSET),
        end_line: get_u32(bytes, UNRESOLVED_SPAN_END_LINE_OFFSET),
        end_column: get_u32(bytes, UNRESOLVED_SPAN_END_COLUMN_OFFSET),
    }
}
