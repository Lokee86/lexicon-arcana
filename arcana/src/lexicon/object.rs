use serde_json::{Map, Value};

use super::LexiconSnapshotError;
use super::format::JsonFactObject;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FactObject {
    pub(super) version: u64,
    pub(super) language: String,
    pub(super) owner: Option<String>,
    pub(super) source_content_id: Option<String>,
    pub(super) adapter_version: String,
    pub(super) schema_version: u64,
    pub(super) analysis_config_id: String,
    pub(super) records: Vec<FactRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum FactRecord {
    Node(NodeRecord),
    Edge(EdgeRecord),
    Unresolved(UnresolvedRecord),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RecordCounts {
    pub(super) nodes: usize,
    pub(super) edges: usize,
    pub(super) unresolved: usize,
}

impl RecordCounts {
    pub(super) fn from_records(records: &[FactRecord]) -> Self {
        let mut counts = Self::default();
        for record in records {
            match record {
                FactRecord::Node(_) => counts.nodes += 1,
                FactRecord::Edge(_) => counts.edges += 1,
                FactRecord::Unresolved(_) => counts.unresolved += 1,
            }
        }
        counts
    }

    pub(super) fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            nodes: self.nodes.checked_add(other.nodes)?,
            edges: self.edges.checked_add(other.edges)?,
            unresolved: self.unresolved.checked_add(other.unresolved)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RecordSelection {
    All,
    Nodes,
    Relations,
}

impl RecordSelection {
    pub(super) const fn includes(self, record: &FactRecord) -> bool {
        matches!(
            (self, record),
            (Self::All, _)
                | (Self::Nodes, FactRecord::Node(_))
                | (
                    Self::Relations,
                    FactRecord::Edge(_) | FactRecord::Unresolved(_)
                )
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SpanRecord {
    pub(super) path: String,
    pub(super) start_line: u64,
    pub(super) start_column: u64,
    pub(super) end_line: u64,
    pub(super) end_column: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NodeRecord {
    pub(super) attributes: Option<Vec<u8>>,
    pub(super) content_id: Option<String>,
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) owner: Option<String>,
    pub(super) path: String,
    pub(super) qualified_name: String,
    pub(super) span: Option<SpanRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EdgeRecord {
    pub(super) attributes: Option<Vec<u8>>,
    pub(super) owner: Option<String>,
    pub(super) relation: String,
    pub(super) source: String,
    pub(super) span: Option<SpanRecord>,
    pub(super) target: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct UnresolvedRecord {
    pub(super) attributes: Option<Vec<u8>>,
    pub(super) candidate_name: Option<String>,
    pub(super) candidate_namespace: Option<String>,
    pub(super) expression: String,
    pub(super) owner: Option<String>,
    pub(super) reason: String,
    pub(super) relation: String,
    pub(super) source: String,
    pub(super) span: Option<SpanRecord>,
}

pub(super) fn parse_json_object(bytes: &[u8]) -> Result<FactObject, LexiconSnapshotError> {
    let object: JsonFactObject = serde_json::from_slice(bytes)?;
    let records = object
        .records
        .into_iter()
        .map(parse_json_record)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FactObject {
        version: object.version,
        language: object.language,
        owner: object.owner,
        source_content_id: object.source_content_id,
        adapter_version: object.adapter_version,
        schema_version: object.schema_version,
        analysis_config_id: object.analysis_config_id,
        records,
    })
}

fn parse_json_record(value: Value) -> Result<FactRecord, LexiconSnapshotError> {
    let object = value
        .as_object()
        .ok_or(LexiconSnapshotError::Malformed("fact object record"))?;
    match required_string(object, "record")? {
        "node" => Ok(FactRecord::Node(NodeRecord {
            attributes: attributes(object)?,
            content_id: optional_string(object, "content_id")?,
            id: required_string(object, "id")?.to_owned(),
            kind: required_string(object, "kind")?.to_owned(),
            name: required_string(object, "name")?.to_owned(),
            owner: optional_string(object, "owner")?,
            path: required_string(object, "path")?.to_owned(),
            qualified_name: required_string(object, "qualified_name")?.to_owned(),
            span: span(object.get("span"))?,
        })),
        "edge" => Ok(FactRecord::Edge(EdgeRecord {
            attributes: attributes(object)?,
            owner: optional_string(object, "owner")?,
            relation: required_string(object, "relation")?.to_owned(),
            source: required_string(object, "source")?.to_owned(),
            span: span(object.get("span"))?,
            target: required_string(object, "target")?.to_owned(),
        })),
        "unresolved" => Ok(FactRecord::Unresolved(UnresolvedRecord {
            attributes: attributes(object)?,
            candidate_name: optional_string(object, "candidate_name")?,
            candidate_namespace: optional_string(object, "candidate_namespace")?,
            expression: required_string(object, "expression")?.to_owned(),
            owner: optional_string(object, "owner")?,
            reason: required_string(object, "reason")?.to_owned(),
            relation: required_string(object, "relation")?.to_owned(),
            source: required_string(object, "source")?.to_owned(),
            span: span(object.get("span"))?,
        })),
        _ => Err(LexiconSnapshotError::Malformed("fact object record")),
    }
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
) -> Result<&'a str, LexiconSnapshotError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or(LexiconSnapshotError::Malformed(field))
}

fn optional_string(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, LexiconSnapshotError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(LexiconSnapshotError::Malformed(field)),
    }
}

fn attributes(object: &Map<String, Value>) -> Result<Option<Vec<u8>>, LexiconSnapshotError> {
    match object.get("attributes") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(serde_json::to_vec(value)?)),
    }
}

fn span(value: Option<&Value>) -> Result<Option<SpanRecord>, LexiconSnapshotError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let object = value
        .as_object()
        .ok_or(LexiconSnapshotError::Malformed("source span"))?;
    Ok(Some(SpanRecord {
        path: required_string(object, "path")?.to_owned(),
        start_line: required_u64(object, "start_line")?,
        start_column: required_u64(object, "start_column")?,
        end_line: required_u64(object, "end_line")?,
        end_column: required_u64(object, "end_column")?,
    }))
}

fn required_u64(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<u64, LexiconSnapshotError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(LexiconSnapshotError::Malformed(field))
}
