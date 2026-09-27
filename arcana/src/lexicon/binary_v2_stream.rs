use super::LexiconSnapshotError;
use super::binary_v2::{
    COMMON_NODE_KINDS, COMMON_RELATIONS, Envelope, decode_node_keys, optional_compact_identity,
    parse_envelope,
};
use super::binary_v2_reader::{Reader, SpanRef};
use super::identity::LexiconIdentity;
use super::object::{FactObject, NodeReference, RecordCounts};

pub(super) struct NodeRef<'a> {
    pub(super) attributes: Option<&'a [u8]>,
    pub(super) content_id: Option<LexiconIdentity>,
    pub(super) id: LexiconIdentity,
    pub(super) kind: &'a str,
    pub(super) name: &'a str,
    pub(super) owner: Option<&'a str>,
    pub(super) path: &'a str,
    pub(super) qualified_name: &'a str,
    pub(super) span: Option<SpanRef<'a>>,
}

pub(super) struct EdgeRef<'a> {
    pub(super) owner: Option<&'a str>,
    pub(super) relation: &'a str,
    pub(super) source: NodeReference,
    pub(super) span: Option<SpanRef<'a>>,
    pub(super) target: NodeReference,
}

pub(super) struct UnresolvedRef<'a> {
    pub(super) candidate_name: Option<&'a str>,
    pub(super) candidate_namespace: Option<&'a str>,
    pub(super) expression: &'a str,
    pub(super) owner: Option<&'a str>,
    pub(super) reason: &'a str,
    pub(super) relation: &'a str,
    pub(super) source: NodeReference,
    pub(super) span: Option<SpanRef<'a>>,
}

pub(super) enum RelationRef<'a> {
    Edge(EdgeRef<'a>),
    Unresolved(UnresolvedRef<'a>),
}

pub(super) fn visit_nodes(
    bytes: &[u8],
    mut visit: impl FnMut(NodeRef<'_>) -> Result<(), LexiconSnapshotError>,
) -> Result<(FactObject, RecordCounts), LexiconSnapshotError> {
    let envelope = parse_envelope(bytes, false)?;
    let mut reader = Reader::new(envelope.nodes);
    let count = reader.count("node records", 20_000_000)?;
    for _ in 0..count {
        let attributes = reader.attributes_ref()?;
        let content_id =
            optional_compact_identity(&mut reader, &envelope.strings, "node content ID")?;
        let id = reader.compact_identity(&envelope.strings, "node ID")?;
        let kind = reader.code_or_string_ref(&envelope.strings, COMMON_NODE_KINDS, "node kind")?;
        let name = reader.string_ref(&envelope.strings, "node name")?;
        let owner = optional(reader.factored_ref(
            &envelope.strings,
            &envelope.object_owner,
            "node owner",
        )?);
        let path = reader.factored_ref(&envelope.strings, &envelope.object_owner, "node path")?;
        let qualified_name = reader.qualified_name_ref(
            &envelope.strings,
            name,
            path,
            &envelope.object_owner,
            "node qualified name",
        )?;
        let span = reader.span_ref(&envelope.strings)?;
        visit(NodeRef {
            attributes,
            content_id,
            id,
            kind,
            name,
            owner,
            path,
            qualified_name,
            span,
        })?;
    }
    reader.finish("node section")?;
    let counts = envelope.counts;
    Ok((metadata(envelope), counts))
}

pub(super) fn visit_relations(
    bytes: &[u8],
    mut visit: impl FnMut(RelationRef<'_>) -> Result<(), LexiconSnapshotError>,
) -> Result<(FactObject, RecordCounts), LexiconSnapshotError> {
    let envelope = parse_envelope(bytes, true)?;
    let node_keys = decode_node_keys(envelope.nodes, &envelope.strings)?;

    let mut edges = Reader::new(envelope.edges);
    let edge_count = edges.count("edge records", 20_000_000)?;
    for _ in 0..edge_count {
        edges.skip_attributes()?;
        visit(RelationRef::Edge(EdgeRef {
            owner: optional(edges.factored_ref(
                &envelope.strings,
                &envelope.object_owner,
                "edge owner",
            )?),
            relation: edges.code_or_string_ref(
                &envelope.strings,
                COMMON_RELATIONS,
                "edge relation",
            )?,
            source: edges.node_ref(&node_keys, &envelope.external, "edge source")?,
            span: edges.span_ref(&envelope.strings)?,
            target: edges.node_ref(&node_keys, &envelope.external, "edge target")?,
        }))?;
    }
    edges.finish("edge section")?;

    let mut unresolved = Reader::new(envelope.unresolved);
    let unresolved_count = unresolved.count("unresolved records", 20_000_000)?;
    for _ in 0..unresolved_count {
        unresolved.skip_attributes()?;
        visit(RelationRef::Unresolved(UnresolvedRef {
            candidate_name: optional(unresolved.string_ref(&envelope.strings, "candidate name")?),
            candidate_namespace: optional(
                unresolved.string_ref(&envelope.strings, "candidate namespace")?,
            ),
            expression: unresolved.string_ref(&envelope.strings, "expression")?,
            owner: optional(unresolved.factored_ref(
                &envelope.strings,
                &envelope.object_owner,
                "unresolved owner",
            )?),
            reason: unresolved.string_ref(&envelope.strings, "unresolved reason")?,
            relation: unresolved.code_or_string_ref(
                &envelope.strings,
                COMMON_RELATIONS,
                "unresolved relation",
            )?,
            source: unresolved.node_ref(&node_keys, &envelope.external, "unresolved source")?,
            span: unresolved.span_ref(&envelope.strings)?,
        }))?;
    }
    unresolved.finish("unresolved section")?;

    let counts = envelope.counts;
    Ok((metadata(envelope), counts))
}

fn metadata(envelope: Envelope<'_>) -> FactObject {
    FactObject {
        version: envelope.version,
        language: envelope.language,
        owner: envelope.owner,
        source_content_id: envelope.source_content_id,
        adapter_version: envelope.adapter_version,
        schema_version: envelope.schema_version,
        analysis_config_id: envelope.analysis_config_id,
        records: Vec::new(),
    }
}

fn optional(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}
