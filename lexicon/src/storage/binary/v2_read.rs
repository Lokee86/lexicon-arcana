use super::common::{
    NODE_KINDS, decode_attributes, optional, read_code, read_factored, read_qname,
};
use super::reader::{Reader, binary_error};
use super::v2_fields::{identity, string_table};
use super::{MAGIC_V2, MAX_EXTERNAL_REFERENCES, MAX_RECORDS, MAX_SECTION_SIZE, MAX_STRING_SIZE};
use crate::{FactObject, FactRecord, NodeRecord, StorageError};

struct Parts<'a> {
    object: FactObject,
    strings: Vec<String>,
    external: Vec<String>,
    nodes: &'a [u8],
    edges: &'a [u8],
    unresolved: &'a [u8],
}

pub(crate) fn decode(bytes: &[u8]) -> Result<FactObject, StorageError> {
    let mut parts = parts(bytes)?;
    let decoded_nodes = decode_nodes(parts.nodes, &parts.strings, &parts.object.owner)?;
    let node_ids = decoded_nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    parts
        .object
        .records
        .extend(decoded_nodes.into_iter().map(FactRecord::Node));
    parts.object.records.extend(super::v2_records::decode_edges(
        parts.edges,
        &parts.strings,
        &parts.external,
        &node_ids,
        &parts.object.owner,
    )?);
    parts
        .object
        .records
        .extend(super::v2_records::decode_unresolved(
            parts.unresolved,
            &parts.strings,
            &parts.external,
            &node_ids,
            &parts.object.owner,
        )?);
    Ok(parts.object)
}

pub(crate) fn decode_nodes_only(
    bytes: &[u8],
) -> Result<(FactObject, Vec<NodeRecord>), StorageError> {
    let parts = parts(bytes)?;
    let nodes = decode_nodes(parts.nodes, &parts.strings, &parts.object.owner)?;
    Ok((parts.object, nodes))
}

fn parts(bytes: &[u8]) -> Result<Parts<'_>, StorageError> {
    if !bytes.starts_with(MAGIC_V2) {
        return Err(binary_error("invalid v2 object magic"));
    }
    let mut reader = Reader::at(bytes, MAGIC_V2.len());
    let version = reader.uvarint("object version")?;
    let schema_version = reader.uvarint("schema version")?;
    let strings = string_table(&mut reader)?;
    let language = reader.string_ref(&strings, "language")?.to_owned();
    let owner = reader.string_ref(&strings, "owner")?.to_owned();
    let source_content_id = identity(&mut reader, &strings, "source content ID")?;
    let adapter_version = reader.string_ref(&strings, "adapter version")?.to_owned();
    let analysis_config_id = identity(&mut reader, &strings, "analysis config ID")?;

    let count = reader.count("external references", MAX_EXTERNAL_REFERENCES)?;
    let mut external = Vec::with_capacity(count);
    for index in 0..count {
        external.push(identity(
            &mut reader,
            &strings,
            &format!("external reference {index}"),
        )?);
    }
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
        external,
        nodes,
        edges,
        unresolved,
    })
}

fn decode_nodes(
    bytes: &[u8],
    strings: &[String],
    object_owner: &str,
) -> Result<Vec<NodeRecord>, StorageError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("node records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let attributes = decode_attributes(reader.bytes("record attributes", MAX_STRING_SIZE)?)?;
        let content_id = optional(identity(&mut reader, strings, "node content ID")?);
        let id = identity(&mut reader, strings, "node ID")?;
        let kind = read_code(&mut reader, strings, NODE_KINDS, "node kind")?;
        let name = reader.string_ref(strings, "node name")?.to_owned();
        let owner = optional(read_factored(
            &mut reader,
            strings,
            object_owner,
            "node owner",
        )?);
        let path = read_factored(&mut reader, strings, object_owner, "node path")?;
        let qualified_name = read_qname(
            &mut reader,
            strings,
            &name,
            &path,
            object_owner,
            "node qualified name",
        )?;
        let span = reader.span(strings)?;
        records.push(NodeRecord {
            attributes,
            content_id,
            id,
            kind,
            name,
            owner,
            path,
            qualified_name,
            span,
        });
    }
    reader.finish("node section")?;
    Ok(records)
}

#[cfg(test)]
mod tests {
    use crate::{EdgeRecord, FactObject, FactRecord, NodeRecord};

    use super::{decode, decode_nodes_only, parts};
    #[test]
    fn nodes_only_decode_skips_relationship_payloads() {
        let object = FactObject {
            version: 1,
            language: "python".into(),
            owner: "a.py".into(),
            source_content_id: String::new(),
            adapter_version: "test".into(),
            schema_version: 1,
            analysis_config_id: "config".into(),
            records: vec![
                FactRecord::Node(NodeRecord {
                    attributes: None,
                    content_id: None,
                    id: "node:a".into(),
                    kind: "function".into(),
                    name: "run".into(),
                    owner: Some("a.py".into()),
                    path: "a.py".into(),
                    qualified_name: "a.run".into(),
                    span: None,
                }),
                FactRecord::Edge(EdgeRecord {
                    attributes: None,
                    owner: Some("a.py".into()),
                    relation: "calls".into(),
                    source: "node:a".into(),
                    span: None,
                    target: "node:a".into(),
                }),
            ],
        };
        let mut encoded = super::super::v2_write::encode(&object).expect("encode object");
        let edge_start = {
            let parsed = parts(&encoded).expect("split object");
            assert_eq!(parsed.edges.first().copied(), Some(1));
            parsed.edges.as_ptr() as usize - encoded.as_ptr() as usize
        };

        encoded[edge_start] = 2;
        assert!(decode(&encoded).is_err());
        let (_, nodes) = decode_nodes_only(&encoded).expect("decode nodes only");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "node:a");
    }
}
