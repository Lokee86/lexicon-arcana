use super::FactStream;
use super::incremental;
use super::model::FactRecord;
use super::order;
use super::path;
use crate::FACT_SCHEMA_VERSION;
use crate::identity::validate_sha256_id;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    EmptyStream,
    Json(String),
    RecordJson(usize, String),
    MissingRecordKind(usize),
    UnsupportedRecord(String),
    InvalidHeader(&'static str),
    InvalidMode(String),
    InvalidIdentity(String),
    InvalidPath(String),
    InvalidSpan(String),
    NonCanonicalOrder,
    ConflictingNode(String),
    UnknownSource(String),
    InvalidIncrementalOwnership(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStream => write!(formatter, "Lexicon fact stream is empty"),
            Self::Json(error) => write!(formatter, "invalid Lexicon JSON: {error}"),
            Self::RecordJson(line, error) => {
                write!(formatter, "invalid Lexicon record on line {line}: {error}")
            }
            Self::MissingRecordKind(line) => write!(formatter, "record on line {line} has no kind"),
            Self::UnsupportedRecord(record) => write!(formatter, "unsupported record {record:?}"),
            Self::InvalidHeader(field) => write!(formatter, "invalid Lexicon header field {field}"),
            Self::InvalidMode(mode) => write!(formatter, "unsupported Lexicon mode {mode:?}"),
            Self::InvalidIdentity(value) => write!(formatter, "invalid Lexicon identity {value:?}"),
            Self::InvalidPath(path) => write!(formatter, "invalid repository path {path:?}"),
            Self::InvalidSpan(path) => write!(formatter, "invalid source span for {path:?}"),
            Self::NonCanonicalOrder => {
                write!(formatter, "Lexicon facts are not canonically ordered")
            }
            Self::ConflictingNode(id) => write!(formatter, "conflicting node identity {id}"),
            Self::UnknownSource(id) => write!(formatter, "unknown relationship source {id}"),
            Self::InvalidIncrementalOwnership(path) => {
                write!(formatter, "incremental fact has invalid ownership {path:?}")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

pub(crate) fn stream(stream: &FactStream) -> Result<(), ValidationError> {
    header(stream)?;
    let mut nodes = BTreeMap::new();
    let mut owners = BTreeMap::new();

    for record in &stream.records {
        record_fields(record)?;
        if let FactRecord::Node(node) = record {
            if let Some(existing) = nodes.insert(node.id.clone(), node)
                && existing != node
            {
                return Err(ValidationError::ConflictingNode(node.id.clone()));
            }
            if let Some(owner) = incremental::direct_owner(record) {
                owners.insert(node.id.clone(), owner.to_owned());
            }
        }
    }

    if stream
        .records
        .windows(2)
        .any(|pair| order::compare(&pair[0], &pair[1]).is_gt())
    {
        return Err(ValidationError::NonCanonicalOrder);
    }

    for record in &stream.records {
        let source = match record {
            FactRecord::Edge(edge) => Some(edge.source.as_str()),
            FactRecord::Unresolved(value) => Some(value.source.as_str()),
            FactRecord::Node(_) => None,
        };
        if let Some(source) = source
            && !nodes.contains_key(source)
            && stream.header.language != "interstack"
        {
            return Err(ValidationError::UnknownSource(source.to_owned()));
        }
    }

    if stream.header.mode.as_deref() == Some("incremental") {
        incremental::validate(stream, &owners)?;
    }
    Ok(())
}

fn header(stream: &FactStream) -> Result<(), ValidationError> {
    let header = &stream.header;
    if header.record != "lexicon" {
        return Err(ValidationError::InvalidHeader("record"));
    }
    if header.schema_version != FACT_SCHEMA_VERSION {
        return Err(ValidationError::InvalidHeader("schema_version"));
    }
    for (field, value) in [
        ("adapter_version", header.adapter_version.as_str()),
        ("language", header.language.as_str()),
        ("repository", header.repository.as_str()),
    ] {
        if value.is_empty() {
            return Err(ValidationError::InvalidHeader(field));
        }
    }
    match header.mode.as_deref() {
        None | Some("full") => {}
        Some("incremental") => {
            if header.changed_files.is_none() {
                return Err(ValidationError::InvalidHeader("changed_files"));
            }
            if header.removed_files.is_none() {
                return Err(ValidationError::InvalidHeader("removed_files"));
            }
            if header.shared_complete.is_none() {
                return Err(ValidationError::InvalidHeader("shared_complete"));
            }
        }
        Some(mode) => return Err(ValidationError::InvalidMode(mode.to_owned())),
    }
    for paths in [&header.changed_files, &header.removed_files]
        .into_iter()
        .flatten()
    {
        path::sorted_paths(paths)?;
    }
    Ok(())
}

fn record_fields(record: &FactRecord) -> Result<(), ValidationError> {
    if let Some(owner) = record.owner() {
        path::repository_path(owner, false)?;
    }
    if let Some(span) = record.span() {
        path::source_span(span)?;
    }
    match record {
        FactRecord::Node(node) => {
            identity(&node.id)?;
            if let Some(content_id) = &node.content_id {
                identity(content_id)?;
            }
            if node.path == "." {
                if node.owner.is_some() || node.span.is_some() || node.kind == "file" {
                    return Err(ValidationError::InvalidPath(node.path.clone()));
                }
            } else {
                path::repository_path(&node.path, true)?;
            }
        }
        FactRecord::Edge(edge) => {
            identity(&edge.source)?;
            identity(&edge.target)?;
        }
        FactRecord::Unresolved(value) => {
            identity(&value.source)?;
        }
    }
    Ok(())
}

fn identity(value: &str) -> Result<(), ValidationError> {
    validate_sha256_id(value).map_err(|_| ValidationError::InvalidIdentity(value.to_owned()))
}
