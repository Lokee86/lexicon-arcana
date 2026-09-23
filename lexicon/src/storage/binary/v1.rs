use super::common::{decode_attributes, optional};
use super::reader::{Reader, binary_error};
use super::{MAGIC_V1, MAX_RECORDS, MAX_SECTION_SIZE, MAX_STRING_SIZE};
use crate::{FactObject, FactRecord, NodeRecord, StorageError};

struct Parts<'a> {
    object: FactObject,
    strings: Vec<String>,
    nodes: &'a [u8],
    edges: &'a [u8],
    unresolved: &'a [u8],
}

pub(crate) fn decode(bytes: &[u8]) -> Result<FactObject, StorageError> {
    let mut parts = parts(bytes)?;
    parts.object.records.extend(
        decode_nodes(parts.nodes, &parts.strings)?
            .into_iter()
            .map(FactRecord::Node),
    );
    parts.object.records.extend(super::v1_records::decode_edges(
        parts.edges,
        &parts.strings,
    )?);
    parts
        .object
        .records
        .extend(super::v1_records::decode_unresolved(
            parts.unresolved,
            &parts.strings,
        )?);
    Ok(parts.object)
}

pub(crate) fn decode_nodes_only(
    bytes: &[u8],
) -> Result<(FactObject, Vec<NodeRecord>), StorageError> {
    let parts = parts(bytes)?;
    let nodes = decode_nodes(parts.nodes, &parts.strings)?;
    Ok((parts.object, nodes))
}

fn parts(bytes: &[u8]) -> Result<Parts<'_>, StorageError> {
    if !bytes.starts_with(MAGIC_V1) {
        return Err(binary_error("invalid v1 object magic"));
    }
    let mut reader = Reader::at(bytes, MAGIC_V1.len());
    let version = reader.uvarint("object version")?;
    let schema_version = reader.uvarint("schema version")?;
    let strings = reader.strings_v1()?;
    let language = reader.string_ref(&strings, "language")?.to_owned();
    let owner = reader.string_ref(&strings, "owner")?.to_owned();
    let source_content_id = reader.string_ref(&strings, "source content ID")?.to_owned();
    let adapter_version = reader.string_ref(&strings, "adapter version")?.to_owned();
    let analysis_config_id = reader
        .string_ref(&strings, "analysis config ID")?
        .to_owned();
    let nodes = reader.bytes("node section", MAX_SECTION_SIZE)?;
    let edges = reader.bytes("edge section", MAX_SECTION_SIZE)?;
    let unresolved = reader.bytes("unresolved section", MAX_SECTION_SIZE)?;
    reader.finish("fact object")?;
    Ok(Parts {
        object: FactObject {
            version,
            language,
            owner,
            source_content_id,
            adapter_version,
            schema_version,
            analysis_config_id,
            records: Vec::new(),
        },
        strings,
        nodes,
        edges,
        unresolved,
    })
}

fn decode_nodes(bytes: &[u8], strings: &[String]) -> Result<Vec<NodeRecord>, StorageError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("node records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(NodeRecord {
            attributes: decode_attributes(reader.bytes("record attributes", MAX_STRING_SIZE)?)?,
            content_id: optional(reader.string_ref(strings, "node content ID")?.to_owned()),
            id: reader.string_ref(strings, "node ID")?.to_owned(),
            kind: reader.string_ref(strings, "node kind")?.to_owned(),
            name: reader.string_ref(strings, "node name")?.to_owned(),
            owner: optional(reader.string_ref(strings, "node owner")?.to_owned()),
            path: reader.string_ref(strings, "node path")?.to_owned(),
            qualified_name: reader
                .string_ref(strings, "node qualified name")?
                .to_owned(),
            span: reader.span(strings)?,
        });
    }
    reader.finish("node section")?;
    Ok(records)
}
