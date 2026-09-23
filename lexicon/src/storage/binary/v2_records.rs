use super::common::{RELATIONS, decode_attributes, optional, read_code, read_factored};
use super::reader::{Reader, binary_error};
use super::{MAX_RECORDS, MAX_STRING_SIZE};
use crate::{EdgeRecord, FactRecord, StorageError, UnresolvedRecord};

pub(crate) fn decode_edges(
    bytes: &[u8],
    strings: &[String],
    external: &[String],
    node_ids: &[String],
    object_owner: &str,
) -> Result<Vec<FactRecord>, StorageError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("edge records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: decode_attributes(reader.bytes("record attributes", MAX_STRING_SIZE)?)?,
            owner: optional(read_factored(
                &mut reader,
                strings,
                object_owner,
                "edge owner",
            )?),
            relation: read_code(&mut reader, strings, RELATIONS, "edge relation")?,
            source: node_ref(&mut reader, node_ids, external, "edge source")?,
            span: reader.span(strings)?,
            target: node_ref(&mut reader, node_ids, external, "edge target")?,
        }));
    }
    reader.finish("edge section")?;
    Ok(records)
}

pub(crate) fn decode_unresolved(
    bytes: &[u8],
    strings: &[String],
    external: &[String],
    node_ids: &[String],
    object_owner: &str,
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
            owner: optional(read_factored(
                &mut reader,
                strings,
                object_owner,
                "unresolved owner",
            )?),
            reason: reader.string_ref(strings, "unresolved reason")?.to_owned(),
            relation: read_code(&mut reader, strings, RELATIONS, "unresolved relation")?,
            source: node_ref(&mut reader, node_ids, external, "unresolved source")?,
            span: reader.span(strings)?,
        }));
    }
    reader.finish("unresolved section")?;
    Ok(records)
}

fn node_ref(
    reader: &mut Reader<'_>,
    node_ids: &[String],
    external: &[String],
    field: &str,
) -> Result<String, StorageError> {
    let tag = reader.byte(&format!("{field} tag"))?;
    let index = reader.uvarint(&format!("{field} index"))?;
    if index == 0 {
        return Err(binary_error(format!("{field} index is out of range")));
    }
    let index =
        usize::try_from(index - 1).map_err(|_| binary_error(format!("{field} index overflows")))?;
    match tag {
        0 => node_ids
            .get(index)
            .cloned()
            .ok_or_else(|| binary_error(format!("{field} ordinal is out of range"))),
        1 => external
            .get(index)
            .cloned()
            .ok_or_else(|| binary_error(format!("{field} external index is out of range"))),
        _ => Err(binary_error(format!("invalid {field} reference tag {tag}"))),
    }
}
