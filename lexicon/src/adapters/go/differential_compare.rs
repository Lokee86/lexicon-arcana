use std::collections::{BTreeMap, BTreeSet};

use crate::{EdgeRecord, FactRecord, NodeRecord, UnresolvedRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum DifferenceKind {
    MissingNode,
    ExtraNode,
    IdentityMismatch,
    MissingEdge,
    RelationMismatch,
    UnresolvedMismatch,
    SpanMismatch,
    AttributeMismatch,
}

impl DifferenceKind {
    fn label(self) -> &'static str {
        match self {
            Self::MissingNode => "missing node",
            Self::ExtraNode => "extra node",
            Self::IdentityMismatch => "identity mismatch",
            Self::MissingEdge => "missing edge",
            Self::RelationMismatch => "relation mismatch",
            Self::UnresolvedMismatch => "unresolved mismatch",
            Self::SpanMismatch => "span mismatch",
            Self::AttributeMismatch => "attribute mismatch",
        }
    }
}

#[derive(Debug)]
pub(super) struct Difference {
    pub(super) kind: DifferenceKind,
    pub(super) detail: String,
}

pub(super) fn compare_records(expected: &[FactRecord], actual: &[FactRecord]) -> Vec<Difference> {
    let mut differences = Vec::new();
    let id_map = compare_nodes(expected, actual, &mut differences);
    compare_edges(expected, actual, &id_map, &mut differences);
    compare_unresolved(expected, actual, &id_map, &mut differences);
    differences
}

fn compare_nodes(
    expected: &[FactRecord],
    actual: &[FactRecord],
    differences: &mut Vec<Difference>,
) -> BTreeMap<String, String> {
    let expected_nodes = nodes_by_id(expected);
    let actual_nodes = nodes_by_id(actual);
    let mut actual_to_expected = BTreeMap::new();
    let mut unmatched_expected = BTreeSet::new();
    let mut unmatched_actual = BTreeSet::new();

    for (id, expected) in &expected_nodes {
        if let Some(actual) = actual_nodes.get(id) {
            actual_to_expected.insert(id.clone(), id.clone());
            compare_node(expected, actual, differences);
        } else {
            unmatched_expected.insert(id.clone());
        }
    }
    for id in actual_nodes.keys() {
        if !expected_nodes.contains_key(id) {
            unmatched_actual.insert(id.clone());
        }
    }

    let mut expected_by_locator = BTreeMap::<String, Vec<String>>::new();
    let mut actual_by_locator = BTreeMap::<String, Vec<String>>::new();
    for id in &unmatched_expected {
        expected_by_locator
            .entry(node_locator(expected_nodes[id]))
            .or_default()
            .push(id.clone());
    }
    for id in &unmatched_actual {
        actual_by_locator
            .entry(node_locator(actual_nodes[id]))
            .or_default()
            .push(id.clone());
    }
    for (locator, expected_ids) in expected_by_locator {
        let Some(actual_ids) = actual_by_locator.get(&locator) else {
            continue;
        };
        if expected_ids.len() != actual_ids.len() {
            continue;
        }
        for (expected_id, actual_id) in expected_ids.iter().zip(actual_ids) {
            unmatched_expected.remove(expected_id);
            unmatched_actual.remove(actual_id);
            actual_to_expected.insert(actual_id.clone(), expected_id.clone());
            differences.push(Difference {
                kind: DifferenceKind::IdentityMismatch,
                detail: format!("{locator}: legacy id {expected_id}, native id {actual_id}"),
            });
            compare_node(
                expected_nodes[expected_id],
                actual_nodes[actual_id],
                differences,
            );
        }
    }

    for id in unmatched_expected {
        differences.push(Difference {
            kind: DifferenceKind::MissingNode,
            detail: format!("native missing {}", node_summary(expected_nodes[&id])),
        });
    }
    for id in unmatched_actual {
        differences.push(Difference {
            kind: DifferenceKind::ExtraNode,
            detail: format!("native-only {}", node_summary(actual_nodes[&id])),
        });
    }
    actual_to_expected
}

fn compare_node(expected: &NodeRecord, actual: &NodeRecord, differences: &mut Vec<Difference>) {
    if expected.kind != actual.kind
        || expected.name != actual.name
        || expected.owner != actual.owner
        || expected.path != actual.path
        || expected.qualified_name != actual.qualified_name
        || expected.content_id != actual.content_id
    {
        differences.push(Difference {
            kind: DifferenceKind::IdentityMismatch,
            detail: format!("node {} differs outside id/span/attributes", expected.id),
        });
    }
    if expected.span != actual.span {
        differences.push(Difference {
            kind: DifferenceKind::SpanMismatch,
            detail: format!(
                "node {}: {:?} != {:?}",
                expected.id, expected.span, actual.span
            ),
        });
    }
    if expected.attributes != actual.attributes {
        differences.push(Difference {
            kind: DifferenceKind::AttributeMismatch,
            detail: format!(
                "node {}: {:?} != {:?}",
                expected.id, expected.attributes, actual.attributes
            ),
        });
    }
}

fn compare_edges(
    expected: &[FactRecord],
    actual: &[FactRecord],
    id_map: &BTreeMap<String, String>,
    differences: &mut Vec<Difference>,
) {
    let expected = edge_groups(expected, &BTreeMap::new());
    let actual = edge_groups(actual, id_map);
    let endpoints = expected
        .keys()
        .chain(actual.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for endpoint in endpoints {
        let left = expected.get(&endpoint).cloned().unwrap_or_default();
        let right = actual.get(&endpoint).cloned().unwrap_or_default();
        if left.is_empty() || right.is_empty() {
            differences.push(Difference {
                kind: DifferenceKind::MissingEdge,
                detail: format!("{endpoint}: legacy={} native={}", left.len(), right.len()),
            });
            continue;
        }
        let left_relations = relation_groups(left);
        let right_relations = relation_groups(right);
        let relations = left_relations
            .keys()
            .chain(right_relations.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for relation in relations {
            let mut left = left_relations.get(&relation).cloned().unwrap_or_default();
            let mut right = right_relations.get(&relation).cloned().unwrap_or_default();
            if left.is_empty() || right.is_empty() {
                differences.push(Difference {
                    kind: DifferenceKind::RelationMismatch,
                    detail: format!(
                        "{endpoint}: relation {relation} legacy={} native={}",
                        left.len(),
                        right.len()
                    ),
                });
                continue;
            }
            left.sort_by_key(edge_detail_key);
            right.sort_by_key(edge_detail_key);
            let paired = left.len().min(right.len());
            for index in 0..paired {
                compare_edge(&left[index], &right[index], differences);
            }
            if left.len() != right.len() {
                differences.push(Difference {
                    kind: DifferenceKind::MissingEdge,
                    detail: format!(
                        "{endpoint} {relation}: legacy={} native={}",
                        left.len(),
                        right.len()
                    ),
                });
            }
        }
    }
}

fn compare_edge(expected: &EdgeRecord, actual: &EdgeRecord, differences: &mut Vec<Difference>) {
    if expected.owner != actual.owner {
        differences.push(Difference {
            kind: DifferenceKind::AttributeMismatch,
            detail: format!(
                "edge {} -> {} {} owner: {:?} != {:?}",
                expected.source, expected.target, expected.relation, expected.owner, actual.owner
            ),
        });
    }
    if expected.span != actual.span {
        differences.push(Difference {
            kind: DifferenceKind::SpanMismatch,
            detail: format!(
                "edge {} -> {} {} span: {:?} != {:?}",
                expected.source, expected.target, expected.relation, expected.span, actual.span
            ),
        });
    }
    if expected.attributes != actual.attributes {
        differences.push(Difference {
            kind: DifferenceKind::AttributeMismatch,
            detail: format!(
                "edge {} -> {} {} attributes: {:?} != {:?}",
                expected.source,
                expected.target,
                expected.relation,
                expected.attributes,
                actual.attributes
            ),
        });
    }
}

fn compare_unresolved(
    expected: &[FactRecord],
    actual: &[FactRecord],
    id_map: &BTreeMap<String, String>,
    differences: &mut Vec<Difference>,
) {
    let expected = unresolved_groups(expected, &BTreeMap::new());
    let actual = unresolved_groups(actual, id_map);
    let keys = expected
        .keys()
        .chain(actual.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for key in keys {
        let mut left = expected.get(&key).cloned().unwrap_or_default();
        let mut right = actual.get(&key).cloned().unwrap_or_default();
        left.sort_by_key(unresolved_detail_key);
        right.sort_by_key(unresolved_detail_key);
        if left.len() != right.len() {
            differences.push(Difference {
                kind: DifferenceKind::UnresolvedMismatch,
                detail: format!("{key}: legacy={} native={}", left.len(), right.len()),
            });
        }
        for index in 0..left.len().min(right.len()) {
            let expected = &left[index];
            let actual = &right[index];
            if expected.reason != actual.reason
                || expected.candidate_name != actual.candidate_name
                || expected.candidate_namespace != actual.candidate_namespace
            {
                differences.push(Difference {
                    kind: DifferenceKind::UnresolvedMismatch,
                    detail: format!("{key}: {:?} != {:?}", expected.reason, actual.reason),
                });
            }
            if expected.span != actual.span {
                differences.push(Difference {
                    kind: DifferenceKind::SpanMismatch,
                    detail: format!("unresolved {key}: {:?} != {:?}", expected.span, actual.span),
                });
            }
            if expected.owner != actual.owner || expected.attributes != actual.attributes {
                differences.push(Difference {
                    kind: DifferenceKind::AttributeMismatch,
                    detail: format!("unresolved {key}: owner/attributes differ"),
                });
            }
        }
    }
}

fn nodes_by_id(records: &[FactRecord]) -> BTreeMap<String, &NodeRecord> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some((node.id.clone(), node)),
            _ => None,
        })
        .collect()
}

fn edge_groups(
    records: &[FactRecord],
    id_map: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<EdgeRecord>> {
    let mut result = BTreeMap::<String, Vec<EdgeRecord>>::new();
    for record in records {
        let FactRecord::Edge(edge) = record else {
            continue;
        };
        let mut edge = edge.clone();
        edge.source = remap(&edge.source, id_map);
        edge.target = remap(&edge.target, id_map);
        result
            .entry(format!("{} -> {}", edge.source, edge.target))
            .or_default()
            .push(edge);
    }
    result
}

fn relation_groups(edges: Vec<EdgeRecord>) -> BTreeMap<String, Vec<EdgeRecord>> {
    let mut result = BTreeMap::<String, Vec<EdgeRecord>>::new();
    for edge in edges {
        result.entry(edge.relation.clone()).or_default().push(edge);
    }
    result
}

fn unresolved_groups(
    records: &[FactRecord],
    id_map: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<UnresolvedRecord>> {
    let mut result = BTreeMap::<String, Vec<UnresolvedRecord>>::new();
    for record in records {
        let FactRecord::Unresolved(value) = record else {
            continue;
        };
        let mut value = value.clone();
        value.source = remap(&value.source, id_map);
        result
            .entry(format!(
                "{} {} {}",
                value.source, value.relation, value.expression
            ))
            .or_default()
            .push(value);
    }
    result
}

fn remap(id: &str, id_map: &BTreeMap<String, String>) -> String {
    id_map.get(id).cloned().unwrap_or_else(|| id.to_owned())
}

fn node_locator(node: &NodeRecord) -> String {
    format!(
        "{}|{}|{}|{}",
        node.kind, node.path, node.name, node.qualified_name
    )
}

fn node_summary(node: &NodeRecord) -> String {
    format!("{} {} {} ({})", node.kind, node.path, node.name, node.id)
}

fn edge_detail_key(edge: &EdgeRecord) -> String {
    serde_json::to_string(&(edge.owner.as_ref(), &edge.span, &edge.attributes)).unwrap()
}

fn unresolved_detail_key(value: &UnresolvedRecord) -> String {
    serde_json::to_string(&(
        &value.reason,
        &value.candidate_namespace,
        &value.candidate_name,
        &value.owner,
        &value.span,
        &value.attributes,
    ))
    .unwrap()
}

#[test]
fn report_uses_stable_difference_categories() {
    let differences = [
        DifferenceKind::MissingNode,
        DifferenceKind::ExtraNode,
        DifferenceKind::IdentityMismatch,
        DifferenceKind::MissingEdge,
        DifferenceKind::RelationMismatch,
        DifferenceKind::UnresolvedMismatch,
        DifferenceKind::SpanMismatch,
        DifferenceKind::AttributeMismatch,
    ]
    .into_iter()
    .map(|kind| Difference {
        kind,
        detail: "example".into(),
    })
    .collect::<Vec<_>>();
    let report = format_differences(&differences);
    for difference in differences {
        assert!(report.contains(difference.kind.label()));
    }
}

pub(super) fn format_differences(differences: &[Difference]) -> String {
    let mut counts = BTreeMap::<DifferenceKind, usize>::new();
    for difference in differences {
        *counts.entry(difference.kind).or_default() += 1;
    }
    let summary = counts
        .iter()
        .map(|(kind, count)| format!("{}={count}", kind.label()))
        .collect::<Vec<_>>()
        .join(", ");
    let details = differences
        .iter()
        .take(24)
        .map(|difference| format!("- {}: {}", difference.kind.label(), difference.detail))
        .collect::<Vec<_>>()
        .join("\n");
    if differences.len() > 24 {
        format!(
            "{summary}\n{details}\n- ... {} additional differences",
            differences.len() - 24
        )
    } else {
        format!("{summary}\n{details}")
    }
}
