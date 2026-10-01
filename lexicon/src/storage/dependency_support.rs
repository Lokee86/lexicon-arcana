use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

#[cfg(test)]
pub(super) type Graph = BTreeMap<String, BTreeSet<String>>;

pub(super) fn collect_dependency_records(
    records: &[FactRecord],
    owner: &str,
    node_owners: &mut BTreeMap<String, String>,
    unresolved_candidates: &mut BTreeSet<String>,
) {
    for record in records {
        match record {
            FactRecord::Node(node) => {
                node_owners.insert(node.id.clone(), owner.to_owned());
            }
            FactRecord::Unresolved(value)
                if addition_sensitive_unresolved(&value.reason)
                    && value
                        .candidate_name
                        .as_deref()
                        .is_some_and(|candidate| !candidate.trim().is_empty()) =>
            {
                unresolved_candidates.insert(
                    value
                        .candidate_name
                        .as_deref()
                        .expect("checked candidate")
                        .trim()
                        .to_owned(),
                );
            }
            _ => {}
        }
    }
}

fn addition_sensitive_unresolved(reason: &str) -> bool {
    matches!(
        reason,
        "missing-target" | "ambiguous-target" | "generated-target" | "external-target"
    )
}

pub(super) fn python_module_candidate(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let stem = normalized.strip_suffix(".py")?;
    let mut parts = stem.split('/').collect::<Vec<_>>();
    if parts.last().copied() == Some("__init__") {
        parts.pop();
    }
    (!parts.is_empty()).then(|| parts.join("."))
}

pub(super) fn normalize_owner(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

#[cfg(test)]
pub(super) fn one_hop_closure(seeds: &BTreeSet<String>, graph: &Graph) -> Vec<String> {
    let mut selected = seeds.clone();
    for seed in seeds {
        if let Some(next) = graph.get(seed) {
            selected.extend(next.iter().cloned());
        }
    }
    selected.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::{Graph, addition_sensitive_unresolved, one_hop_closure, python_module_candidate};
    use std::collections::BTreeSet;

    #[test]
    fn dependency_scope_is_one_hop() {
        let graph = Graph::from([
            ("a.py".into(), BTreeSet::from(["b.py".into()])),
            ("b.py".into(), BTreeSet::from(["c.py".into()])),
        ]);
        let seeds = BTreeSet::from(["a.py".into()]);
        assert_eq!(one_hop_closure(&seeds, &graph), vec!["a.py", "b.py"]);
    }

    #[test]
    fn python_addition_module_candidates_match_import_names() {
        assert_eq!(
            python_module_candidate("pkg/new.py").as_deref(),
            Some("pkg.new")
        );
        assert_eq!(
            python_module_candidate("pkg/new/__init__.py").as_deref(),
            Some("pkg.new")
        );
        assert_eq!(python_module_candidate("README.md"), None);
    }

    #[test]
    fn external_targets_are_addition_sensitive() {
        assert!(addition_sensitive_unresolved("external-target"));
    }
}
