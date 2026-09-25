use std::collections::{BTreeMap, BTreeSet};

use super::super::facts::Facts;
use super::super::source::dotted;
use super::bindings::resolve_reference;

pub fn resolve_inheritance(facts: &mut Facts) {
    let inheritances = facts.inheritances.clone();
    for info in inheritances {
        let reference = dotted(&info.base);
        let (target, reason) = resolve_reference(
            facts,
            &info.module_name,
            Some(&info.class_qname),
            reference.as_deref(),
            None,
        );
        if let Some(target) = target {
            let relation = if facts
                .nodes
                .get(&target)
                .is_some_and(|node| matches!(node.kind.as_str(), "interface" | "trait"))
            {
                "implements"
            } else {
                "extends"
            };
            facts.add_edge(&info.source_id, &target, relation, info.span.clone(), None);
        } else {
            facts.add_unresolved(
                &info.source_id,
                "extends",
                &info.expression,
                &reason,
                info.span.clone(),
                reference,
            );
        }
    }
}

pub fn emit_overrides(facts: &mut Facts) {
    let functions = facts.functions.values().cloned().collect::<Vec<_>>();
    let mut cache = BTreeMap::new();

    for function in functions {
        let Some(class_qname) = &function.class_qname else {
            continue;
        };
        let method = function.qname.rsplit('.').next().unwrap_or(&function.qname);
        let ancestors = ancestors(facts, class_qname, &mut cache, &mut BTreeSet::new());
        for ancestor in ancestors {
            let qname = format!("{ancestor}.{method}");
            let Some(target) = facts.symbols.get(&qname).cloned() else {
                continue;
            };
            if target != function.node_id
                && facts
                    .nodes
                    .get(&target)
                    .is_some_and(|node| node.kind == "method")
            {
                facts.add_edge(&function.node_id, &target, "overrides", None, None);
            }
        }
    }
}

pub fn base_qnames(facts: &Facts, class_qname: &str) -> Vec<String> {
    let Some(info) = facts.classes.get(class_qname) else {
        return Vec::new();
    };
    info.bases
        .iter()
        .filter_map(|base| {
            let reference = dotted(base)?;
            let (target, _) = resolve_reference(
                facts,
                &info.module_name,
                Some(class_qname),
                Some(&reference),
                None,
            );
            target.and_then(|id| {
                facts.nodes.get(&id).and_then(|node| {
                    matches!(node.kind.as_str(), "type" | "interface" | "trait")
                        .then(|| node.qualified_name.clone())
                })
            })
        })
        .collect()
}

pub fn mro_qnames(facts: &Facts, class_qname: &str) -> Vec<String> {
    fn walk(facts: &Facts, class_qname: &str, active: &mut BTreeSet<String>) -> Vec<String> {
        if !active.insert(class_qname.to_owned()) {
            return vec![class_qname.to_owned()];
        }
        let bases = base_qnames(facts, class_qname);
        if bases.is_empty() {
            active.remove(class_qname);
            return vec![class_qname.to_owned()];
        }

        let mut sequences = bases
            .iter()
            .map(|base| walk(facts, base, active))
            .collect::<Vec<_>>();
        sequences.push(bases.clone());
        let mut merged = Vec::new();

        while sequences.iter().any(|sequence| !sequence.is_empty()) {
            sequences.retain(|sequence| !sequence.is_empty());
            let candidate = sequences
                .iter()
                .find_map(|sequence| {
                    let head = sequence.first()?;
                    sequences
                        .iter()
                        .all(|other| !other.iter().skip(1).any(|item| item == head))
                        .then(|| head.clone())
                })
                .unwrap_or_else(|| sequences[0][0].clone());
            if !merged.contains(&candidate) {
                merged.push(candidate.clone());
            }
            for sequence in &mut sequences {
                if sequence.first() == Some(&candidate) {
                    sequence.remove(0);
                }
            }
        }
        active.remove(class_qname);
        std::iter::once(class_qname.to_owned())
            .chain(merged)
            .collect()
    }

    walk(facts, class_qname, &mut BTreeSet::new())
}

pub fn descendants(facts: &Facts, class_qname: &str) -> BTreeSet<String> {
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for candidate in facts.classes.keys() {
        for base in base_qnames(facts, candidate) {
            children.entry(base).or_default().push(candidate.clone());
        }
    }

    let mut result = BTreeSet::new();
    let mut pending = children.get(class_qname).cloned().unwrap_or_default();
    while let Some(candidate) = pending.pop() {
        if !result.insert(candidate.clone()) {
            continue;
        }
        pending.extend(children.get(&candidate).cloned().unwrap_or_default());
    }
    result
}

fn ancestors(
    facts: &Facts,
    class_qname: &str,
    cache: &mut BTreeMap<String, Vec<String>>,
    seen: &mut BTreeSet<String>,
) -> Vec<String> {
    if let Some(values) = cache.get(class_qname) {
        return values.clone();
    }
    if !seen.insert(class_qname.to_owned()) {
        return Vec::new();
    }
    let mut result = Vec::new();
    for base in base_qnames(facts, class_qname) {
        if !result.contains(&base) {
            result.push(base.clone());
        }
        for ancestor in ancestors(facts, &base, cache, seen) {
            if !result.contains(&ancestor) {
                result.push(ancestor);
            }
        }
    }
    seen.remove(class_qname);
    cache.insert(class_qname.to_owned(), result.clone());
    result
}
