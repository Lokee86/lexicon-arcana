use crate::{AdapterError, AdapterRequest, FactRecord, NodeRecord, SourceSpan};

use super::{
    dependencies,
    discovery::Inventory,
    identities,
    protocol_records::{DeclarationKind, Record, Span},
    semantic_call_facts, semantic_capture_facts, semantic_dataflow_facts,
    semantic_fact_index::FactIndex,
    semantic_facts_support::{container_id, parent_id, push_edge, required},
    semantic_relationship_facts,
};

pub(crate) fn add(
    request: &AdapterRequest,
    inventory: &Inventory,
    semantic: &[Record],
    records: &mut Vec<FactRecord>,
) -> Result<(u64, u64), AdapterError> {
    let mut index = FactIndex::from_records(records);

    for record in semantic {
        if matches!(record, Record::Diagnostic { .. }) {
            continue;
        }
        if semantic_capture_facts::add(record, records, &mut index)? {
            continue;
        }
        if semantic_relationship_facts::add(record, inventory, records, &mut index)? {
            continue;
        }
        if semantic_call_facts::add(record, inventory, records, &mut index)? {
            continue;
        }
        if semantic_dataflow_facts::add(record, records, &mut index)? {
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
        let id = index.node_id_for_kind(identity, fact_kind(*kind))?;
        let (path, qualified_name) = node_location(*kind, name, owner, metadata)?;

        index.push_node(
            records,
            NodeRecord {
                attributes: None,
                content_id: None,
                id: id.clone(),
                kind: fact_kind(*kind).into(),
                name: name.clone(),
                owner: Some(owner.clone()),
                path,
                qualified_name,
                span: Some(location.clone()),
            },
        );

        match kind {
            DeclarationKind::Package => {
                let parent = parent_id(owner, inventory, &mut index)?;
                push_edge(
                    records,
                    &mut index,
                    parent,
                    id.clone(),
                    "contains",
                    None,
                    None,
                );
                let file = index.node_id(&identities::file(owner))?;
                push_edge(
                    records,
                    &mut index,
                    id,
                    file,
                    "contains",
                    Some(owner.clone()),
                    Some(location),
                );
            }
            DeclarationKind::Import => {
                let container = container_id(metadata, &mut index)?;
                push_edge(
                    records,
                    &mut index,
                    container,
                    id,
                    "imports",
                    Some(owner.clone()),
                    Some(location),
                );
            }
            DeclarationKind::Namespace => {}
            _ => {
                let container = container_id(metadata, &mut index)?;
                push_edge(
                    records,
                    &mut index,
                    container,
                    id,
                    "defines",
                    Some(owner.clone()),
                    Some(location),
                );
            }
        }
    }
    let dependency_started = crate::perf::start();
    dependencies::add(request, inventory, semantic, records, &mut index)?;
    if let Some(dependency_started) = dependency_started {
        crate::perf::emit(
            "go.dependency_construction",
            dependency_started.elapsed(),
            &[("final_fact_count", records.len() as u64)],
        );
    }
    Ok(index.identity_cache_stats())
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
