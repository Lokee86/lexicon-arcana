use std::collections::{BTreeMap, BTreeSet};

use super::super::facts::Facts;
use super::super::source::dotted;
use super::bindings::BindingResolver;

pub fn resolve_inheritance(facts: &mut Facts, bindings: &mut BindingResolver) {
    let inheritances = std::mem::take(&mut facts.inheritances);
    for info in inheritances {
        let reference = dotted(&info.base);
        let (target, reason) = bindings.resolve_reference(
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
            facts.add_edge(&info.source_id, &target, relation, info.span, None);
        } else {
            facts.add_unresolved(
                &info.source_id,
                "extends",
                &info.expression,
                &reason,
                info.span,
                reference,
            );
        }
    }
}

pub fn emit_overrides(facts: &mut Facts, bindings: &mut BindingResolver) {
    let functions = facts
        .functions
        .values()
        .filter_map(|function| {
            let class_qname = function.class_qname.as_ref()?;
            Some((
                function.node_id.clone(),
                class_qname.clone(),
                function
                    .qname
                    .rsplit('.')
                    .next()
                    .unwrap_or(&function.qname)
                    .to_owned(),
            ))
        })
        .collect::<Vec<_>>();
    let mut cache = BTreeMap::new();

    for (node_id, class_qname, method) in functions {
        let ancestors = ancestors(
            facts,
            bindings,
            &class_qname,
            &mut cache,
            &mut BTreeSet::new(),
        );
        for ancestor in ancestors {
            let qname = format!("{ancestor}.{method}");
            let Some(target) = facts.symbols.get(&qname).cloned() else {
                continue;
            };
            if target != node_id
                && facts
                    .nodes
                    .get(&target)
                    .is_some_and(|node| node.kind == "method")
            {
                facts.add_edge(&node_id, &target, "overrides", None, None);
            }
        }
    }
}

pub fn base_qnames(
    facts: &Facts,
    bindings: &mut BindingResolver,
    class_qname: &str,
) -> Vec<String> {
    let Some(info) = facts.classes.get(class_qname) else {
        return Vec::new();
    };
    info.bases
        .iter()
        .filter_map(|base| {
            let reference = dotted(base)?;
            let (target, _) = bindings.resolve_reference(
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

pub fn mro_qnames(facts: &Facts, bindings: &mut BindingResolver, class_qname: &str) -> Vec<String> {
    fn walk(
        facts: &Facts,
        bindings: &mut BindingResolver,
        class_qname: &str,
        active: &mut BTreeSet<String>,
    ) -> Vec<String> {
        if !active.insert(class_qname.to_owned()) {
            return vec![class_qname.to_owned()];
        }
        let bases = base_qnames(facts, bindings, class_qname);
        if bases.is_empty() {
            active.remove(class_qname);
            return vec![class_qname.to_owned()];
        }

        let mut sequences = bases
            .iter()
            .map(|base| walk(facts, bindings, base, active))
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

    walk(facts, bindings, class_qname, &mut BTreeSet::new())
}

fn ancestors(
    facts: &Facts,
    bindings: &mut BindingResolver,
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
    for base in base_qnames(facts, bindings, class_qname) {
        if !result.contains(&base) {
            result.push(base.clone());
        }
        for ancestor in ancestors(facts, bindings, &base, cache, seen) {
            if !result.contains(&ancestor) {
                result.push(ancestor);
            }
        }
    }
    seen.remove(class_qname);
    cache.insert(class_qname.to_owned(), result.clone());
    result
}
