use super::LexiconSnapshotError;
use super::binary_v2_reader::Reader;
use super::identity::LexiconIdentity;
use super::object::{
    EdgeRecord, FactObject, FactRecord, NodeRecord, RecordCounts, RecordSelection, UnresolvedRecord,
};
use crate::repository::NodeKey;

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

pub(super) fn parse_binary_object_selected(
    bytes: &[u8],
    selection: RecordSelection,
) -> Result<(FactObject, RecordCounts), LexiconSnapshotError> {
    let envelope = parse_envelope(bytes, selection != RecordSelection::Nodes)?;
    let mut records = match selection {
        RecordSelection::All => {
            decode_nodes(envelope.nodes, &envelope.strings, &envelope.object_owner)?
        }
        RecordSelection::Nodes => {
            decode_nodes(envelope.nodes, &envelope.strings, &envelope.object_owner)?
        }
        RecordSelection::Relations => Vec::with_capacity(
            envelope
                .counts
                .edges
                .checked_add(envelope.counts.unresolved)
                .ok_or(LexiconSnapshotError::Malformed("record count overflow"))?,
        ),
    };

    if selection != RecordSelection::Nodes {
        let node_keys = if selection == RecordSelection::All {
            records
                .iter()
                .filter_map(|record| match record {
                    FactRecord::Node(node) => Some(node.id.node_key()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        } else {
            decode_node_keys(envelope.nodes, &envelope.strings)?
        };
        records.extend(decode_edges(
            envelope.edges,
            &envelope.strings,
            &envelope.external,
            &node_keys,
            &envelope.object_owner,
        )?);
        records.extend(decode_unresolved(
            envelope.unresolved,
            &envelope.strings,
            &envelope.external,
            &node_keys,
            &envelope.object_owner,
        )?);
    }

    Ok((
        FactObject {
            version: envelope.version,
            language: envelope.language,
            owner: envelope.owner,
            source_content_id: envelope.source_content_id,
            adapter_version: envelope.adapter_version,
            schema_version: envelope.schema_version,
            analysis_config_id: envelope.analysis_config_id,
            records,
        },
        envelope.counts,
    ))
}

struct Envelope<'a> {
    version: u64,
    schema_version: u64,
    strings: Vec<String>,
    language: String,
    object_owner: String,
    owner: Option<String>,
    source_content_id: Option<LexiconIdentity>,
    adapter_version: String,
    analysis_config_id: LexiconIdentity,
    external: Vec<LexiconIdentity>,
    nodes: &'a [u8],
    edges: &'a [u8],
    unresolved: &'a [u8],
    counts: RecordCounts,
}

fn parse_envelope(
    bytes: &[u8],
    retain_external: bool,
) -> Result<Envelope<'_>, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    reader.expect_magic()?;
    let version = reader.uvarint("object version")?;
    let schema_version = reader.uvarint("schema version")?;
    let strings = reader.string_table()?;
    let language = reader.string_ref(&strings, "language")?.to_owned();
    let object_owner = reader.string_ref(&strings, "owner")?.to_owned();
    let owner = optional(&object_owner);
    let source_content_id = optional_compact_identity(&mut reader, &strings, "source content ID")?;
    let adapter_version = reader.string_ref(&strings, "adapter version")?.to_owned();
    let analysis_config_id = reader.compact_identity(&strings, "analysis config ID")?;

    let external_count = reader.count("external references", MAX_EXTERNAL_REFERENCES)?;
    let mut external = if retain_external {
        Vec::with_capacity(external_count)
    } else {
        Vec::new()
    };
    for index in 0..external_count {
        let field = format!("external reference {index}");
        if retain_external {
            external.push(reader.compact_identity(&strings, &field)?);
        } else {
            reader.skip_identity(&strings, &field)?;
        }
    }

    let nodes = reader.bytes("node section", MAX_SECTION_SIZE)?;
    let edges = reader.bytes("edge section", MAX_SECTION_SIZE)?;
    let unresolved = reader.bytes("unresolved section", MAX_SECTION_SIZE)?;
    reader.finish("fact object")?;

    let counts = RecordCounts {
        nodes: section_record_count(nodes, "node records")?,
        edges: section_record_count(edges, "edge records")?,
        unresolved: section_record_count(unresolved, "unresolved records")?,
    };

    Ok(Envelope {
        version,
        schema_version,
        strings,
        language,
        object_owner,
        owner,
        source_content_id,
        adapter_version,
        analysis_config_id,
        external,
        nodes,
        edges,
        unresolved,
        counts,
    })
}

fn section_record_count(bytes: &[u8], field: &str) -> Result<usize, LexiconSnapshotError> {
    Reader::new(bytes).count(field, MAX_RECORDS)
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
        let content_id = optional_compact_identity(&mut reader, strings, "node content ID")?;
        let id = reader.compact_identity(strings, "node ID")?;
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

fn decode_node_keys(
    bytes: &[u8],
    strings: &[String],
) -> Result<Vec<NodeKey>, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("node records", MAX_RECORDS)?;
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        reader.skip_attributes()?;
        reader.skip_identity(strings, "node content ID")?;
        keys.push(reader.compact_identity(strings, "node ID")?.node_key());
        reader.skip_code_or_string(strings, COMMON_NODE_KINDS, "node kind")?;
        reader.string_ref(strings, "node name")?;
        reader.skip_factored(strings, "node owner")?;
        reader.skip_factored(strings, "node path")?;
        reader.skip_qualified_name(strings, "node qualified name")?;
        reader.skip_span(strings)?;
    }
    reader.finish("node section")?;
    Ok(keys)
}

fn decode_edges(
    bytes: &[u8],
    strings: &[String],
    external: &[LexiconIdentity],
    node_keys: &[NodeKey],
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
            source: reader.node_ref(node_keys, external, "edge source")?,
            span: reader.span(strings)?,
            target: reader.node_ref(node_keys, external, "edge target")?,
        }));
    }
    reader.finish("edge section")?;
    Ok(records)
}

fn decode_unresolved(
    bytes: &[u8],
    strings: &[String],
    external: &[LexiconIdentity],
    node_keys: &[NodeKey],
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
            source: reader.node_ref(node_keys, external, "unresolved source")?,
            span: reader.span(strings)?,
        }));
    }
    reader.finish("unresolved section")?;
    Ok(records)
}

fn optional_compact_identity(
    reader: &mut Reader<'_>,
    strings: &[String],
    field: &str,
) -> Result<Option<LexiconIdentity>, LexiconSnapshotError> {
    match reader.byte(&format!("{field} tag"))? {
        0 => {
            let value = reader.string_ref(strings, field)?;
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(LexiconIdentity::parse(value)?))
            }
        }
        1 => {
            let digest: [u8; 32] = reader.take(32, field)?.try_into().map_err(|_| {
                LexiconSnapshotError::Binary(format!("invalid {field} digest length"))
            })?;
            Ok(Some(LexiconIdentity::from_digest(digest)))
        }
        tag => Err(LexiconSnapshotError::Binary(format!(
            "invalid {field} identity tag {tag}"
        ))),
    }
}

fn optional(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn optional_owned(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
