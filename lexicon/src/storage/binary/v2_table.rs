use super::common::{NODE_KINDS, RELATIONS, code, collect_span, is_sha256};
use super::view::ObjectView;
use crate::FactRecord;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(crate) struct Table {
    pub(crate) values: Vec<String>,
    pub(crate) index: BTreeMap<String, u64>,
}

pub(crate) struct References {
    pub(crate) local: HashMap<String, u64>,
    pub(crate) external: Vec<String>,
    pub(crate) external_index: HashMap<String, u64>,
}

pub(crate) fn build(object: &ObjectView<'_>) -> (Table, References) {
    let local = local_ids(object);
    let mut strings = BTreeSet::new();
    for value in [object.language, object.owner, object.adapter_version] {
        strings.insert(value.to_owned());
    }
    for value in [object.source_content_id, object.analysis_config_id] {
        if !is_sha256(value) {
            strings.insert(value.to_owned());
        }
    }
    collect_record_strings(&mut strings, object, &local);

    let mut values = vec![String::new()];
    values.extend(strings.into_iter().filter(|value| !value.is_empty()));
    let index = values
        .iter()
        .enumerate()
        .map(|(index, value)| (value.clone(), index as u64))
        .collect();

    let (external, external_index) = external_refs(object, &local);
    (
        Table { values, index },
        References {
            local,
            external,
            external_index,
        },
    )
}

fn local_ids(object: &ObjectView<'_>) -> HashMap<String, u64> {
    object
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some(node.id.clone()),
            _ => None,
        })
        .enumerate()
        .map(|(index, id)| (id, index as u64))
        .collect()
}

fn collect_record_strings(
    strings: &mut BTreeSet<String>,
    object: &ObjectView<'_>,
    local: &HashMap<String, u64>,
) {
    for record in object.records.iter() {
        match record {
            FactRecord::Node(node) => {
                for identity in [&node.content_id, &Some(node.id.clone())]
                    .into_iter()
                    .flatten()
                {
                    if !is_sha256(identity) {
                        strings.insert(identity.clone());
                    }
                }
                if code(&node.kind, NODE_KINDS) == 0 {
                    strings.insert(node.kind.clone());
                }
                strings.insert(node.name.clone());
                insert_factored(strings, node.owner.as_deref().unwrap_or(""), object.owner);
                insert_factored(strings, &node.path, object.owner);
                if node.qualified_name != node.name
                    && node.qualified_name != node.path
                    && node.qualified_name != object.owner
                {
                    strings.insert(node.qualified_name.clone());
                }
                collect_span(strings, node.span.as_ref());
            }
            FactRecord::Edge(edge) => {
                insert_factored(strings, edge.owner.as_deref().unwrap_or(""), object.owner);
                insert_relation(strings, &edge.relation);
                insert_external_identity(strings, local, &edge.source);
                insert_external_identity(strings, local, &edge.target);
                collect_span(strings, edge.span.as_ref());
            }
            FactRecord::Unresolved(value) => {
                for text in [
                    value.candidate_name.as_deref().unwrap_or(""),
                    value.candidate_namespace.as_deref().unwrap_or(""),
                    &value.expression,
                    &value.reason,
                ] {
                    strings.insert(text.to_owned());
                }
                insert_factored(strings, value.owner.as_deref().unwrap_or(""), object.owner);
                insert_relation(strings, &value.relation);
                insert_external_identity(strings, local, &value.source);
                collect_span(strings, value.span.as_ref());
            }
        }
    }
}

fn insert_factored(strings: &mut BTreeSet<String>, value: &str, object_owner: &str) {
    if !value.is_empty() && value != object_owner {
        strings.insert(value.to_owned());
    }
}

fn insert_relation(strings: &mut BTreeSet<String>, value: &str) {
    if code(value, RELATIONS) == 0 {
        strings.insert(value.to_owned());
    }
}

fn insert_external_identity(
    strings: &mut BTreeSet<String>,
    local: &HashMap<String, u64>,
    value: &str,
) {
    if !local.contains_key(value) && !is_sha256(value) {
        strings.insert(value.to_owned());
    }
}

fn external_refs(
    object: &ObjectView<'_>,
    local: &HashMap<String, u64>,
) -> (Vec<String>, HashMap<String, u64>) {
    let mut values = Vec::new();
    let mut index = HashMap::new();
    let mut add = |value: &str| {
        if local.contains_key(value) || index.contains_key(value) {
            return;
        }
        index.insert(value.to_owned(), values.len() as u64);
        values.push(value.to_owned());
    };
    for record in object.records.iter() {
        match record {
            FactRecord::Edge(edge) => {
                add(&edge.source);
                add(&edge.target);
            }
            FactRecord::Unresolved(value) => add(&value.source),
            FactRecord::Node(_) => {}
        }
    }
    (values, index)
}
