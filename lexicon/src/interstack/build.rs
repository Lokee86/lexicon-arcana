use std::path::Path;

use serde_json::{Map, Value};

use crate::{
    ANALYSIS_CONFIG_ID, Analysis, FactHeader, FactRecord, FactStream, LanguageEntry, NodeRecord,
    ScanExecutionError, SnapshotManifest, Store,
};

use super::{ADAPTER_VERSION, LANGUAGE, Library, Node, Summary, adapter_fingerprint, resolve};

pub fn interstack_drifted(manifest: &SnapshotManifest) -> bool {
    let languages = manifest.languages.as_deref().unwrap_or_default();
    let ordinary_count = languages
        .iter()
        .filter(|entry| entry.language != LANGUAGE)
        .count();
    let derived = languages.iter().find(|entry| entry.language == LANGUAGE);
    if ordinary_count == 0 {
        return derived.is_some();
    }
    let Some(derived) = derived else {
        return true;
    };
    derived.adapter_version != ADAPTER_VERSION
        || derived.adapter_fingerprint != adapter_fingerprint()
        || derived.schema_version != 1
}

pub fn refresh_interstack(
    store: &Store,
    source_root: &Path,
    manifest: SnapshotManifest,
) -> Result<(SnapshotManifest, Summary), ScanExecutionError> {
    let ordinary = manifest
        .languages
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|entry| entry.language != LANGUAGE)
        .cloned()
        .collect::<Vec<_>>();
    if ordinary.is_empty() {
        return Ok((manifest.without_language(LANGUAGE), Summary::default()));
    }

    let libraries = ordinary
        .iter()
        .map(|entry| library_from_entry(store, entry))
        .collect::<Result<Vec<_>, _>>()?;
    let result = resolve(source_root, &libraries)?;
    let summary = result.summary.clone();
    let analysis = analysis_from_result(result)?;
    let entry =
        store.build_shared_language(&analysis, ANALYSIS_CONFIG_ID, &adapter_fingerprint())?;
    Ok((manifest.with_language(entry), summary))
}

fn library_from_entry(store: &Store, entry: &LanguageEntry) -> Result<Library, ScanExecutionError> {
    let mut nodes = Vec::new();
    for file in entry.files.as_deref().unwrap_or_default() {
        collect_nodes(store, &file.object_id, &mut nodes)?;
    }
    if !entry.shared_object_id.is_empty() {
        collect_nodes(store, &entry.shared_object_id, &mut nodes)?;
    }
    Ok(Library {
        language: entry.language.clone(),
        repository: entry.repository.clone(),
        nodes,
    })
}

fn collect_nodes(
    store: &Store,
    object_id: &str,
    nodes: &mut Vec<Node>,
) -> Result<(), ScanExecutionError> {
    let (_, loaded) = store.load_node_facts(object_id)?;
    nodes.extend(loaded.into_iter().map(Node::from));
    Ok(())
}

fn analysis_from_result(result: super::ResolveResult) -> Result<Analysis, ScanExecutionError> {
    let header = FactHeader {
        adapter_version: ADAPTER_VERSION.into(),
        changed_files: None,
        language: LANGUAGE.into(),
        mode: Some("full".into()),
        record: "lexicon".into(),
        removed_files: None,
        repository: if result.repository.is_empty() {
            "repository".into()
        } else {
            result.repository
        },
        schema_version: 1,
        shared_complete: None,
    };
    let mut records =
        Vec::with_capacity(result.nodes.len() + result.edges.len() + result.unresolved.len());
    records.extend(result.nodes.into_iter().map(|node| {
        FactRecord::Node(NodeRecord {
            attributes: map_value(node.attributes),
            content_id: None,
            id: node.id,
            kind: node.kind,
            name: node.name,
            owner: None,
            path: node.path,
            qualified_name: node.qualified_name,
            span: node.span,
        })
    }));
    records.extend(result.edges.into_iter().map(FactRecord::Edge));
    records.extend(result.unresolved.into_iter().map(FactRecord::Unresolved));

    let mut stream = FactStream { header, records };
    stream.sort_records();
    stream
        .validate()
        .map_err(|error| ScanExecutionError::new(error.to_string()))?;
    Ok(Analysis {
        header: stream.header,
        records: stream.records,
    })
}

fn map_value(values: super::model::Attributes) -> Option<Value> {
    if values.is_empty() {
        None
    } else {
        Some(Value::Object(values.into_iter().collect::<Map<_, _>>()))
    }
}
