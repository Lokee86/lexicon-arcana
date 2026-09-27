use crate::repository::{ContentId, NodeFact, NodeKey};

use super::format::{
    NODE_CONTENT_ID_OFFSET, NODE_EXTERNAL_IDENTITY_OFFSET, NODE_FLAG_CONTENT_ID,
    NODE_FLAG_EXTERNAL_IDENTITY, NODE_FLAG_SPAN, NODE_FLAGS_OFFSET, NODE_KEY_OFFSET,
    NODE_KIND_OFFSET, NODE_NAME_ID_OFFSET, NODE_OCCURRENCE_COUNT_OFFSET, NODE_PATH_ID_OFFSET,
    NODE_QUALIFIED_NAME_ID_OFFSET, NODE_RECORD_LEN, NODE_SPAN_END_COLUMN_OFFSET,
    NODE_SPAN_END_LINE_OFFSET, NODE_SPAN_PATH_ID_OFFSET, NODE_SPAN_START_COLUMN_OFFSET,
    NODE_SPAN_START_LINE_OFFSET, node_kind_code, node_kind_from_code,
};
use super::record_io::{get_u16, get_u32, get_u64, put_u16, put_u32, put_u64};
use super::{
    CompactSpan, CompactStringTable, Sha256Identity, StoreFormatError, StringId, StringIdLookup,
};

const KNOWN_FLAGS: u16 = NODE_FLAG_EXTERNAL_IDENTITY | NODE_FLAG_CONTENT_ID | NODE_FLAG_SPAN;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactNodeRecord {
    pub key: NodeKey,
    pub external_identity: Option<Sha256Identity>,
    pub content_id: Option<ContentId>,
    pub path: StringId,
    pub name: StringId,
    pub qualified_name: StringId,
    pub span: Option<CompactSpan>,
    pub occurrence_count: u32,
    pub kind_code: u16,
}

impl CompactNodeRecord {
    pub(crate) fn from_fact(
        fact: &NodeFact,
        strings: &(impl StringIdLookup + ?Sized),
        occurrence_count: u32,
    ) -> Result<Self, StoreFormatError> {
        if occurrence_count == 0 {
            return Err(StoreFormatError::InvalidOccurrenceCount);
        }
        Ok(Self {
            key: fact.key,
            external_identity: fact
                .external_identity
                .as_deref()
                .map(Sha256Identity::parse)
                .transpose()?,
            content_id: fact.content_id,
            path: strings.id(&fact.path)?,
            name: strings.id(&fact.name)?,
            qualified_name: strings.id(&fact.qualified_name)?,
            span: fact
                .span
                .as_ref()
                .map(|span| CompactSpan::from_source(span, strings))
                .transpose()?,
            occurrence_count,
            kind_code: node_kind_code(&fact.kind),
        })
    }

    pub fn to_fact(self, strings: &CompactStringTable) -> Result<NodeFact, StoreFormatError> {
        Ok(NodeFact {
            key: self.key,
            external_identity: self.external_identity.map(Sha256Identity::canonical_string),
            kind: node_kind_from_code(self.kind_code)
                .ok_or(StoreFormatError::InvalidNodeKind(self.kind_code))?,
            path: strings.get(self.path)?.to_owned(),
            name: strings.get(self.name)?.to_owned(),
            qualified_name: strings.get(self.qualified_name)?.to_owned(),
            content_id: self.content_id,
            span: self.span.map(|span| span.to_source(strings)).transpose()?,
        })
    }

    pub fn encode(self) -> [u8; NODE_RECORD_LEN as usize] {
        let mut bytes = [0_u8; NODE_RECORD_LEN as usize];
        put_u64(&mut bytes, NODE_KEY_OFFSET, self.key.as_u64());
        let mut flags = 0_u16;
        if let Some(identity) = self.external_identity {
            bytes[NODE_EXTERNAL_IDENTITY_OFFSET..NODE_EXTERNAL_IDENTITY_OFFSET + 32]
                .copy_from_slice(&identity.0);
            flags |= NODE_FLAG_EXTERNAL_IDENTITY;
        }
        if let Some(content_id) = self.content_id {
            put_u64(&mut bytes, NODE_CONTENT_ID_OFFSET, content_id.as_u64());
            flags |= NODE_FLAG_CONTENT_ID;
        }
        put_u32(&mut bytes, NODE_PATH_ID_OFFSET, self.path.0);
        put_u32(&mut bytes, NODE_NAME_ID_OFFSET, self.name.0);
        put_u32(
            &mut bytes,
            NODE_QUALIFIED_NAME_ID_OFFSET,
            self.qualified_name.0,
        );
        if let Some(span) = self.span {
            put_span(&mut bytes, span);
            flags |= NODE_FLAG_SPAN;
        } else {
            put_u32(&mut bytes, NODE_SPAN_PATH_ID_OFFSET, StringId::ABSENT.0);
        }
        put_u32(
            &mut bytes,
            NODE_OCCURRENCE_COUNT_OFFSET,
            self.occurrence_count,
        );
        put_u16(&mut bytes, NODE_KIND_OFFSET, self.kind_code);
        put_u16(&mut bytes, NODE_FLAGS_OFFSET, flags);
        bytes
    }

    pub fn decode(bytes: &[u8; NODE_RECORD_LEN as usize]) -> Result<Self, StoreFormatError> {
        let flags = get_u16(bytes, NODE_FLAGS_OFFSET);
        if flags & !KNOWN_FLAGS != 0 {
            return Err(StoreFormatError::InvalidFlags(flags));
        }
        let occurrence_count = get_u32(bytes, NODE_OCCURRENCE_COUNT_OFFSET);
        if occurrence_count == 0 {
            return Err(StoreFormatError::InvalidOccurrenceCount);
        }
        let external_identity = if flags & NODE_FLAG_EXTERNAL_IDENTITY != 0 {
            let mut digest = [0_u8; 32];
            digest.copy_from_slice(
                &bytes[NODE_EXTERNAL_IDENTITY_OFFSET..NODE_EXTERNAL_IDENTITY_OFFSET + 32],
            );
            Some(Sha256Identity(digest))
        } else {
            None
        };
        let span = if flags & NODE_FLAG_SPAN != 0 {
            Some(read_span(bytes))
        } else {
            None
        };
        Ok(Self {
            key: NodeKey::from_u64(get_u64(bytes, NODE_KEY_OFFSET)),
            external_identity,
            content_id: (flags & NODE_FLAG_CONTENT_ID != 0)
                .then(|| ContentId::from_u64(get_u64(bytes, NODE_CONTENT_ID_OFFSET))),
            path: StringId(get_u32(bytes, NODE_PATH_ID_OFFSET)),
            name: StringId(get_u32(bytes, NODE_NAME_ID_OFFSET)),
            qualified_name: StringId(get_u32(bytes, NODE_QUALIFIED_NAME_ID_OFFSET)),
            span,
            occurrence_count,
            kind_code: get_u16(bytes, NODE_KIND_OFFSET),
        })
    }
}

fn put_span(bytes: &mut [u8], span: CompactSpan) {
    put_u32(bytes, NODE_SPAN_PATH_ID_OFFSET, span.path.0);
    put_u32(bytes, NODE_SPAN_START_LINE_OFFSET, span.start_line);
    put_u32(bytes, NODE_SPAN_START_COLUMN_OFFSET, span.start_column);
    put_u32(bytes, NODE_SPAN_END_LINE_OFFSET, span.end_line);
    put_u32(bytes, NODE_SPAN_END_COLUMN_OFFSET, span.end_column);
}

fn read_span(bytes: &[u8]) -> CompactSpan {
    CompactSpan {
        path: StringId(get_u32(bytes, NODE_SPAN_PATH_ID_OFFSET)),
        start_line: get_u32(bytes, NODE_SPAN_START_LINE_OFFSET),
        start_column: get_u32(bytes, NODE_SPAN_START_COLUMN_OFFSET),
        end_line: get_u32(bytes, NODE_SPAN_END_LINE_OFFSET),
        end_column: get_u32(bytes, NODE_SPAN_END_COLUMN_OFFSET),
    }
}
