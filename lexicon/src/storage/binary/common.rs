use super::reader::{Reader, binary_error};
use crate::{FactRecord, SourceSpan, StorageError};
use std::collections::BTreeSet;

pub(crate) const NODE_KINDS: &[&str] = &[
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

pub(crate) const RELATIONS: &[&str] = &[
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

pub(crate) fn code(value: &str, values: &[&str]) -> u64 {
    values
        .iter()
        .position(|candidate| *candidate == value)
        .map_or(0, |index| index as u64 + 1)
}

pub(crate) fn read_code(
    reader: &mut Reader<'_>,
    strings: &[String],
    values: &[&str],
    field: &str,
) -> Result<String, StorageError> {
    let encoded = reader.uvarint(&format!("{field} code"))?;
    if encoded == 0 {
        return Ok(reader.string_ref(strings, field)?.to_owned());
    }
    values
        .get((encoded - 1) as usize)
        .map(|value| (*value).to_owned())
        .ok_or_else(|| binary_error(format!("{field} code is out of range")))
}

pub(crate) fn read_factored(
    reader: &mut Reader<'_>,
    strings: &[String],
    object_owner: &str,
    field: &str,
) -> Result<String, StorageError> {
    match reader.uvarint(&format!("{field} factor"))? {
        0 => Ok(String::new()),
        1 => Ok(object_owner.to_owned()),
        2 => Ok(reader.string_ref(strings, field)?.to_owned()),
        _ => Err(binary_error(format!("invalid {field} factor"))),
    }
}

pub(crate) fn read_qname(
    reader: &mut Reader<'_>,
    strings: &[String],
    name: &str,
    path: &str,
    object_owner: &str,
    field: &str,
) -> Result<String, StorageError> {
    match reader.uvarint(&format!("{field} factor"))? {
        0 => Ok(reader.string_ref(strings, field)?.to_owned()),
        1 => Ok(name.to_owned()),
        2 => Ok(path.to_owned()),
        3 => Ok(object_owner.to_owned()),
        _ => Err(binary_error(format!("invalid {field} factor"))),
    }
}

pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn decode_attributes(bytes: &[u8]) -> Result<Option<serde_json::Value>, StorageError> {
    if bytes.is_empty() {
        Ok(None)
    } else {
        Ok(Some(serde_json::from_slice(bytes)?))
    }
}

pub(crate) fn attributes_bytes(record: &FactRecord) -> Result<Vec<u8>, StorageError> {
    let value = match record {
        FactRecord::Node(value) => value.attributes.as_ref(),
        FactRecord::Edge(value) => value.attributes.as_ref(),
        FactRecord::Unresolved(value) => value.attributes.as_ref(),
    };
    value.map_or_else(
        || Ok(Vec::new()),
        |value| serde_json::to_vec(value).map_err(Into::into),
    )
}

pub(crate) fn collect_span(strings: &mut BTreeSet<String>, span: Option<&SourceSpan>) {
    if let Some(span) = span {
        strings.insert(span.path.clone());
    }
}

pub(crate) fn optional(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
