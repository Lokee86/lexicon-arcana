use super::model::{FactHeader, FactRecord};
use super::validate::ValidationError;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn validate(
    header: &FactHeader,
    records: &[FactRecord],
    node_owners: &BTreeMap<String, String>,
) -> Result<(), ValidationError> {
    let changed: BTreeSet<&str> = header
        .changed_files
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect();
    let removed: BTreeSet<&str> = header
        .removed_files
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect();

    for record in records {
        let owner = direct_owner(record).or_else(|| match record {
            FactRecord::Edge(edge) => node_owners.get(&edge.source).map(String::as_str),
            FactRecord::Unresolved(value) => node_owners.get(&value.source).map(String::as_str),
            FactRecord::Node(_) => None,
        });
        let Some(owner) = owner else {
            continue;
        };
        if removed.contains(owner) || !changed.contains(owner) {
            return Err(ValidationError::InvalidIncrementalOwnership(
                owner.to_owned(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn direct_owner(record: &FactRecord) -> Option<&str> {
    record
        .owner()
        .or_else(|| record.span().map(|span| span.path.as_str()))
        .or_else(|| match record {
            FactRecord::Node(node) if node.kind == "file" => Some(node.path.as_str()),
            _ => None,
        })
}
