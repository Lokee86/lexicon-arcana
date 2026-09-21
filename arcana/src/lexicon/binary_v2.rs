use super::LexiconSnapshotError;
use super::binary_v2_reader::Reader;
use super::object::{EdgeRecord, FactObject, FactRecord, NodeRecord, UnresolvedRecord};

pub(super) const MAGIC: &[u8; 8] = b"LXOBJ\0\x02\0";
const MAX_RECORDS: u64 = 20_000_000;
const MAX_EXTERNAL_REFERENCES: u64 = 4_000_000;
const MAX_SECTION_SIZE: u64 = 512 * 1024 * 1024;

const COMMON_NODE_KINDS: &[&str] = &[
    "repository",
    "directory",
    "file",
    "module",
    "namespace",
    "symbol",
    "type",
    "interface",
    "protocol",
    "trait",
    "function",
    "method",
    "constructor",
    "field",
    "variable",
    "constant",
    "parameter",
    "import",
    "test",
    "http-endpoint",
    "message-channel",
    "config-key",
];

const COMMON_RELATIONS: &[&str] = &[
    "contains",
    "defines",
    "imports",
    "calls",
    "possible-calls",
    "passes-to",
    "converts-to",
    "references",
    "extends",
    "implements",
    "uses-trait",
    "overrides",
    "reads",
    "writes",
    "annotates",
    "includes",
    "depends-on",
    "tests",
    "documents",
    "generates",
    "calls-endpoint",
    "handled-by",
    "publishes",
    "consumes",
    "reads-config",
];

pub(super) fn parse_binary_object(bytes: &[u8]) -> Result<FactObject, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    reader.expect_magic()?;
    let version = reader.uvarint("object version")?;
    let schema_version = reader.uvarint("schema version")?;
    let strings = reader.string_table()?;
    let language = reader.string_ref(&strings, "language")?.to_owned();
    let object_owner = reader.string_ref(&strings, "owner")?.to_owned();
    let owner = optional(&object_owner);
    let source_content_id = optional_owned(reader.identity(&strings, "source content ID")?);
    let adapter_version = reader.string_ref(&strings, "adapter version")?.to_owned();
    let analysis_config_id = reader.identity(&strings, "analysis config ID")?;

    let external_count = reader.count("external references", MAX_EXTERNAL_REFERENCES)?;
    let mut external = Vec::with_capacity(external_count);
    for index in 0..external_count {
        external.push(reader.identity(&strings, &format!("external reference {index}"))?);
    }

    let nodes = reader.bytes("node section", MAX_SECTION_SIZE)?;
    let edges = reader.bytes("edge section", MAX_SECTION_SIZE)?;
    let unresolved = reader.bytes("unresolved section", MAX_SECTION_SIZE)?;
    reader.finish("fact object")?;

    let mut records = decode_nodes(nodes, &strings, &object_owner)?;
    let node_ids = records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some(node.id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    records.extend(decode_edges(
        edges,
        &strings,
        &external,
        &node_ids,
        &object_owner,
    )?);
    records.extend(decode_unresolved(
        unresolved,
        &strings,
        &external,
        &node_ids,
        &object_owner,
    )?);

    Ok(FactObject {
        version,
        language,
        owner,
        source_content_id,
        adapter_version,
        schema_version,
        analysis_config_id,
        records,
    })
}

fn decode_nodes(
    bytes: &[u8],
    strings: &[String],
    object_owner: &str,
) -> Result<Vec<FactRecord>, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("node records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let attributes = reader.attributes()?;
        let content_id = optional_owned(reader.identity(strings, "node content ID")?);
        let id = reader.identity(strings, "node ID")?;
        let kind = reader.code_or_string(strings, COMMON_NODE_KINDS, "node kind")?;
        let name = reader.string_ref(strings, "node name")?.to_owned();
        let owner = optional_owned(reader.factored(strings, object_owner, "node owner")?);
        let path = reader.factored(strings, object_owner, "node path")?;
        let qualified_name =
            reader.qualified_name(strings, &name, &path, object_owner, "node qualified name")?;
        let span = reader.span(strings)?;
        records.push(FactRecord::Node(NodeRecord {
            attributes,
            content_id,
            id,
            kind,
            name,
            owner,
            path,
            qualified_name,
            span,
        }));
    }
    reader.finish("node section")?;
    Ok(records)
}

fn decode_edges(
    bytes: &[u8],
    strings: &[String],
    external: &[String],
    node_ids: &[String],
    object_owner: &str,
) -> Result<Vec<FactRecord>, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("edge records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: reader.attributes()?,
            owner: optional_owned(reader.factored(strings, object_owner, "edge owner")?),
            relation: reader.code_or_string(strings, COMMON_RELATIONS, "edge relation")?,
            source: reader.node_ref(node_ids, external, "edge source")?,
            span: reader.span(strings)?,
            target: reader.node_ref(node_ids, external, "edge target")?,
        }));
    }
    reader.finish("edge section")?;
    Ok(records)
}

fn decode_unresolved(
    bytes: &[u8],
    strings: &[String],
    external: &[String],
    node_ids: &[String],
    object_owner: &str,
) -> Result<Vec<FactRecord>, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("unresolved records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(FactRecord::Unresolved(UnresolvedRecord {
            attributes: reader.attributes()?,
            candidate_name: optional(reader.string_ref(strings, "candidate name")?),
            candidate_namespace: optional(reader.string_ref(strings, "candidate namespace")?),
            expression: reader.string_ref(strings, "expression")?.to_owned(),
            owner: optional_owned(reader.factored(strings, object_owner, "unresolved owner")?),
            reason: reader.string_ref(strings, "unresolved reason")?.to_owned(),
            relation: reader.code_or_string(strings, COMMON_RELATIONS, "unresolved relation")?,
            source: reader.node_ref(node_ids, external, "unresolved source")?,
            span: reader.span(strings)?,
        }));
    }
    reader.finish("unresolved section")?;
    Ok(records)
}

fn optional(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn optional_owned(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
