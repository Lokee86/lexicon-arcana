use sha2::{Digest, Sha256};

use super::LexiconSnapshotError;
use super::binary_v2_reader::SpanRef;
use super::binary_v2_stream::NodeRef;
use super::identity::LexiconIdentity;
use crate::repository_store::{CompactRepositoryAssembler, TempStringId};

const SIGNATURE_DOMAIN: &[u8] = b"arcana.lexicon.node-signature.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NodeSignature {
    attributes: Option<Box<[u8]>>,
    content_id: Option<LexiconIdentity>,
    kind: TempStringId,
    name: TempStringId,
    owner: Option<TempStringId>,
    path: TempStringId,
    qualified_name: TempStringId,
    span: Option<RawSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawSpan {
    path: TempStringId,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
}

pub(super) fn signature(
    assembler: &mut CompactRepositoryAssembler,
    record: &NodeRef<'_>,
) -> Result<NodeSignature, LexiconSnapshotError> {
    Ok(NodeSignature {
        attributes: record
            .attributes
            .map(<[u8]>::to_vec)
            .map(Vec::into_boxed_slice),
        content_id: record.content_id,
        kind: assembler.intern(record.kind)?,
        name: assembler.intern(record.name)?,
        owner: optional_intern(assembler, record.owner)?,
        path: assembler.intern(record.path)?,
        qualified_name: assembler.intern(record.qualified_name)?,
        span: raw_span(assembler, record.span)?,
    })
}

pub(super) fn signature_digest(record: &NodeRef<'_>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SIGNATURE_DOMAIN);
    optional_bytes(&mut hasher, 1, record.attributes);
    optional_identity(&mut hasher, 2, record.content_id);
    bytes(&mut hasher, 3, record.kind.as_bytes());
    bytes(&mut hasher, 4, record.name.as_bytes());
    optional_bytes(&mut hasher, 5, record.owner.map(str::as_bytes));
    bytes(&mut hasher, 6, record.path.as_bytes());
    bytes(&mut hasher, 7, record.qualified_name.as_bytes());
    span(&mut hasher, 8, record.span);
    hasher.finalize().into()
}

pub(super) fn optional_intern(
    assembler: &mut CompactRepositoryAssembler,
    value: Option<&str>,
) -> Result<Option<TempStringId>, LexiconSnapshotError> {
    value
        .map(|value| assembler.intern(value))
        .transpose()
        .map_err(Into::into)
}

fn raw_span(
    assembler: &mut CompactRepositoryAssembler,
    span: Option<SpanRef<'_>>,
) -> Result<Option<RawSpan>, LexiconSnapshotError> {
    span.map(|span| {
        Ok(RawSpan {
            path: assembler.intern(span.path)?,
            start_line: span.start_line,
            start_column: span.start_column,
            end_line: span.end_line,
            end_column: span.end_column,
        })
    })
    .transpose()
}

fn bytes(hasher: &mut Sha256, tag: u8, value: &[u8]) {
    hasher.update([tag]);
    write_bytes(hasher, value);
}

fn optional_bytes(hasher: &mut Sha256, tag: u8, value: Option<&[u8]>) {
    hasher.update([tag]);
    match value {
        None => hasher.update([0]),
        Some(value) => {
            hasher.update([1]);
            write_bytes(hasher, value);
        }
    }
}

fn optional_identity(hasher: &mut Sha256, tag: u8, value: Option<LexiconIdentity>) {
    hasher.update([tag]);
    match value {
        None => hasher.update([0]),
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.digest());
        }
    }
}

fn span(hasher: &mut Sha256, tag: u8, value: Option<SpanRef<'_>>) {
    hasher.update([tag]);
    let Some(value) = value else {
        hasher.update([0]);
        return;
    };
    hasher.update([1]);
    write_bytes(hasher, value.path.as_bytes());
    for number in [
        value.start_line,
        value.start_column,
        value.end_line,
        value.end_column,
    ] {
        hasher.update(number.to_le_bytes());
    }
}

fn write_bytes(hasher: &mut Sha256, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("node signature field length fits u64");
    hasher.update(length.to_le_bytes());
    hasher.update(value);
}
