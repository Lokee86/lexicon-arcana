use std::path::Path;

use crate::{
    AdapterMode, AdapterRequest, Analysis, EdgeRecord, FACT_SCHEMA_VERSION, FactHeader, FactRecord,
    NodeRecord,
};

use super::{
    ADAPTER_VERSION, discovery::Inventory, identities, observations::Observation, semantic_facts,
};

pub(crate) fn structural_analysis(
    request: &AdapterRequest,
    inventory: &Inventory,
    semantic: &[Observation],
) -> Result<Analysis, crate::AdapterError> {
    let materialization_started = crate::perf::start();
    let mut records = Vec::new();
    let repository_id = identities::node_id(&identities::repository(&inventory.repository))?;
    records.push(FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: repository_id.clone(),
        kind: "repository".into(),
        name: inventory.repository.clone(),
        owner: None,
        path: ".lexicon-repository".into(),
        qualified_name: inventory.repository.clone(),
        span: None,
    }));

    for directory in &inventory.directories {
        let id = identities::node_id(&identities::directory(directory))?;
        records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: id.clone(),
            kind: "directory".into(),
            name: file_name(directory),
            owner: None,
            path: directory.clone(),
            qualified_name: directory.clone(),
            span: None,
        }));
        records.push(contains(parent_id(directory, &repository_id)?, id));
    }

    for file in &inventory.files {
        let id = identities::node_id(&identities::file(&file.path))?;
        records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: Some(file.content_id.clone()),
            id: id.clone(),
            kind: "file".into(),
            name: file_name(&file.path),
            owner: Some(file.path.clone()),
            path: file.path.clone(),
            qualified_name: file.path.clone(),
            span: None,
        }));
        records.push(contains(parent_id(&file.path, &repository_id)?, id));
    }
    let (identity_cache_hits, identity_cache_misses) =
        semantic_facts::add(request, inventory, semantic, &mut records)?;

    if let Some(materialization_started) = materialization_started {
        let materialized_nodes = records
            .iter()
            .filter(|record| matches!(record, FactRecord::Node(_)))
            .count() as u64;
        let materialized_edges = records
            .iter()
            .filter(|record| matches!(record, FactRecord::Edge(_)))
            .count() as u64;
        crate::perf::emit(
            "go.lexicon.materialization",
            materialization_started.elapsed(),
            &[
                ("materialized_nodes", materialized_nodes),
                ("materialized_edges", materialized_edges),
                ("identity_cache_hits", identity_cache_hits),
                ("identity_cache_misses", identity_cache_misses),
                ("final_fact_count", records.len() as u64),
            ],
        );
    }

    let incremental = request.mode == AdapterMode::Incremental;
    let mut analysis = Analysis::new(
        FactHeader {
            adapter_version: ADAPTER_VERSION.into(),
            changed_files: incremental.then(|| normalized(&request.changed_files)),
            language: "go".into(),
            mode: Some(if incremental { "incremental" } else { "full" }.into()),
            record: "lexicon".into(),
            removed_files: incremental.then(|| normalized(&request.removed_files)),
            repository: inventory.repository.clone(),
            schema_version: FACT_SCHEMA_VERSION,
            shared_complete: incremental.then_some(true),
        },
        records,
    );
    analysis
        .canonicalize()
        .map_err(|error| crate::AdapterError::new(error.to_string()))?;
    Ok(analysis)
}

fn contains(source: String, target: String) -> FactRecord {
    FactRecord::Edge(EdgeRecord {
        attributes: None,
        owner: None,
        relation: "contains".into(),
        source,
        span: None,
        target,
    })
}

fn parent_id(path: &str, repository: &str) -> Result<String, crate::AdapterError> {
    match Path::new(path).parent().and_then(|value| value.to_str()) {
        Some(parent) if !parent.is_empty() => {
            identities::node_id(&identities::directory(&parent.replace('\\', "/")))
        }
        _ => Ok(repository.to_owned()),
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_owned()
}

fn normalized(paths: &[String]) -> Vec<String> {
    let mut values = paths
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}
