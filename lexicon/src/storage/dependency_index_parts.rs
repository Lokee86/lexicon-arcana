use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

use super::dependency_index_model::FileTopology;
use super::dependency_support::normalize_owner;

/// Preserve exactly the file-owned edge/unresolved semantics of the old
/// dependency_data scanner. Unowned shared edges do not become file edges.
pub(super) fn file_records(records: &[&FactRecord]) -> FileTopology {
    let mut result = FileTopology::default();
    for record in records {
        match record {
            FactRecord::Node(node) => {
                result.nodes.insert(node.id.clone());
            }
            FactRecord::Edge(edge) => {
                result.referenced_nodes.insert(edge.target.clone());
            }
            FactRecord::Unresolved(value)
                if matches!(
                    value.reason.as_str(),
                    "missing-target" | "ambiguous-target" | "generated-target" | "external-target"
                ) && let Some(candidate) = value.candidate_name.as_deref()
                    && !candidate.trim().is_empty() =>
            {
                result
                    .unresolved_candidates
                    .insert(candidate.trim().to_owned());
            }
            _ => {}
        }
    }
    result
}

pub(super) fn shared_nodes(records: &[&FactRecord]) -> BTreeMap<String, BTreeSet<String>> {
    let mut result = BTreeMap::<String, BTreeSet<String>>::new();
    for record in records {
        if let FactRecord::Node(node) = record {
            result
                .entry(normalize_owner(&node.path))
                .or_default()
                .insert(node.id.clone());
        }
    }
    result
}
