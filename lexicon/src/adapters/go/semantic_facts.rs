use std::collections::BTreeSet;

use crate::{AdapterError, FactRecord, NodeRecord, SourceSpan};

use super::{
    discovery::Inventory,
    identities,
    protocol_records::{DeclarationKind, Record, Span},
    semantic_call_facts, semantic_capture_facts, semantic_dataflow_facts,
    semantic_facts_support::{EdgeKey, container_id, parent_id, push_edge, required},
    semantic_relationship_facts,
};

pub(crate) fn add(
    inventory: &Inventory,
    semantic: &[Record],
    records: &mut Vec<FactRecord>,
) -> Result<(), AdapterError> {
    let mut nodes = records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some(node.id.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut edges = BTreeSet::<EdgeKey>::new();

    for record in semantic {
        if matches!(record, Record::Diagnostic { .. }) {
            continue;
        }
        if semantic_capture_facts::add(record, records, &mut nodes, &mut edges)? {
            continue;
        }
        if semantic_relationship_facts::add(record, inventory, records, &mut nodes, &mut edges)? {
            continue;
        }
        if semantic_call_facts::add(record, inventory, records, &mut nodes, &mut edges)? {
            continue;
        }
        if semantic_dataflow_facts::add(record, records, &mut nodes, &mut edges)? {
            continue;
        }
        let Record::Declaration {
            identity,
            kind,
            name,
            owner,
            span,
            metadata,
        } = record
        else {
            return Err(AdapterError::new(
                "non-structural Go semantic record arrived before its migration phase",
            ));
        };
        let location = source_span(owner, span);
        let id = identities::node_id_for_kind(identity, fact_kind(*kind))?;
        let (path, qualified_name) = node_location(*kind, name, owner, metadata)?;

        if nodes.insert(id.clone()) {
            records.push(FactRecord::Node(NodeRecord {
                attributes: None,
                content_id: None,
                id: id.clone(),
                kind: fact_kind(*kind).into(),
                name: name.clone(),
                owner: Some(owner.clone()),
                path,
                qualified_name,
                span: Some(location.clone()),
            }));
        }

        match kind {
            DeclarationKind::Package => {
                push_edge(
                    records,
                    &mut edges,
                    parent_id(owner, inventory)?,
                    id.clone(),
                    "contains",
                    None,
                    None,
                );
                push_edge(
                    records,
                    &mut edges,
                    id,
                    identities::node_id(&identities::file(owner))?,
                    "contains",
                    Some(owner.clone()),
                    Some(location),
                );
            }
            DeclarationKind::Import => {
                push_edge(
                    records,
                    &mut edges,
                    container_id(metadata)?,
                    id,
                    "imports",
                    Some(owner.clone()),
                    Some(location),
                );
            }
            DeclarationKind::Namespace => {}
            _ => {
                push_edge(
                    records,
                    &mut edges,
                    container_id(metadata)?,
                    id,
                    "defines",
                    Some(owner.clone()),
                    Some(location),
                );
            }
        }
    }
    Ok(())
}

fn fact_kind(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Package => "module",
        DeclarationKind::Import => "import",
        DeclarationKind::Namespace => "namespace",
        DeclarationKind::Type => "type",
        DeclarationKind::Function => "function",
        DeclarationKind::Method => "method",
        DeclarationKind::Test => "test",
        DeclarationKind::Parameter => "parameter",
        DeclarationKind::Variable => "variable",
        DeclarationKind::Field => "field",
        DeclarationKind::Constant => "constant",
    }
}

fn node_location(
    kind: DeclarationKind,
    name: &str,
    owner: &str,
    metadata: &std::collections::BTreeMap<String, String>,
) -> Result<(String, String), AdapterError> {
    match kind {
        DeclarationKind::Package => {
            let path = owner
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_owned())
                .unwrap_or_else(|| ".lexicon-repository".into());
            Ok((path.clone(), format!("{path}::{name}")))
        }
        DeclarationKind::Import => {
            let class = required(metadata, "import_class")?;
            let import = required(metadata, "import_path")?;
            let path = format!("@{class}/{import}");
            Ok((path.clone(), path))
        }
        DeclarationKind::Namespace => {
            let path = required(metadata, "path")?.to_owned();
            Ok((path.clone(), path))
        }
        _ => Ok((owner.to_owned(), format!("{owner}::{name}"))),
    }
}

fn source_span(owner: &str, span: &Span) -> SourceSpan {
    SourceSpan {
        path: owner.to_owned(),
        start_line: span.start_line,
        start_column: span.start_column,
        end_line: span.end_line,
        end_column: span.end_column,
    }
}
