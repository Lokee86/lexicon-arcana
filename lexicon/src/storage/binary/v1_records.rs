use super::common::{decode_attributes, optional};
use super::reader::Reader;
use super::{MAX_RECORDS, MAX_STRING_SIZE};
use crate::{EdgeRecord, FactRecord, StorageError, UnresolvedRecord};

pub(crate) fn decode_edges(
    bytes: &[u8],
    strings: &[String],
) -> Result<Vec<FactRecord>, StorageError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("edge records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: decode_attributes(reader.bytes("record attributes", MAX_STRING_SIZE)?)?,
            owner: optional(reader.string_ref(strings, "edge owner")?.to_owned()),
            relation: reader.string_ref(strings, "edge relation")?.to_owned(),
            source: reader.string_ref(strings, "edge source")?.to_owned(),
            span: reader.span(strings)?,
            target: reader.string_ref(strings, "edge target")?.to_owned(),
        }));
    }
    reader.finish("edge section")?;
    Ok(records)
}

pub(crate) fn decode_unresolved(
    bytes: &[u8],
    strings: &[String],
) -> Result<Vec<FactRecord>, StorageError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("unresolved records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(FactRecord::Unresolved(UnresolvedRecord {
            attributes: decode_attributes(reader.bytes("record attributes", MAX_STRING_SIZE)?)?,
            candidate_name: optional(reader.string_ref(strings, "candidate name")?.to_owned()),
            candidate_namespace: optional(
                reader
                    .string_ref(strings, "candidate namespace")?
                    .to_owned(),
            ),
            expression: reader.string_ref(strings, "expression")?.to_owned(),
            owner: optional(reader.string_ref(strings, "unresolved owner")?.to_owned()),
            reason: reader.string_ref(strings, "unresolved reason")?.to_owned(),
            relation: reader
                .string_ref(strings, "unresolved relation")?
                .to_owned(),
            source: reader.string_ref(strings, "unresolved source")?.to_owned(),
            span: reader.span(strings)?,
        }));
    }
    reader.finish("unresolved section")?;
    Ok(records)
}
