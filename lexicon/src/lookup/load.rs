use std::collections::BTreeMap;

use crate::{
    FactObject, FactRecord, LanguageEntry, NodeRecord, SnapshotManifest, StorageError, Store,
};

use super::{LookupError, LookupNode, SnapshotLookup, StoredEdge, StoredUnresolved};

impl SnapshotLookup {
    pub fn load(store: &Store, snapshot: &str) -> Result<Self, LookupError> {
        Self::load_mode(store, snapshot, true)
    }

    pub fn load_nodes(store: &Store, snapshot: &str) -> Result<Self, LookupError> {
        Self::load_mode(store, snapshot, false)
    }

    fn load_mode(
        store: &Store,
        snapshot: &str,
        relationships_loaded: bool,
    ) -> Result<Self, LookupError> {
        let (snapshot_id, manifest) = resolve_snapshot(store, snapshot)?;
        let mut value = Self {
            snapshot_id,
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            unresolved: Vec::new(),
            relationships_loaded,
        };
        let mut languages = manifest.languages.unwrap_or_default();
        languages.sort_by(|left, right| left.language.cmp(&right.language));
        for entry in languages {
            value.load_language(store, &entry)?;
        }
        if relationships_loaded {
            value.validate_sources()?;
            value.sort_evidence();
        }
        Ok(value)
    }

    fn load_language(&mut self, store: &Store, entry: &LanguageEntry) -> Result<(), LookupError> {
        if !entry.shared_object_id.is_empty() {
            self.load_object(store, entry, &entry.shared_object_id, None)?;
        }

        let mut files = entry.files.clone().unwrap_or_default();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        for file in files {
            self.load_object(
                store,
                entry,
                &file.object_id,
                Some((&file.path, &file.content_id)),
            )?;
        }
        Ok(())
    }

    fn load_object(
        &mut self,
        store: &Store,
        entry: &LanguageEntry,
        object_id: &str,
        expected_file: Option<(&str, &str)>,
    ) -> Result<(), LookupError> {
        if self.relationships_loaded {
            let object = store.load_object(object_id)?;
            validate_metadata(&object, entry, expected_file)?;
            self.ingest(&entry.language, object.records)
        } else {
            let (object, nodes) = store.load_node_facts(object_id)?;
            validate_metadata(&object, entry, expected_file)?;
            self.ingest_nodes(&entry.language, nodes)
        }
    }

    fn ingest(&mut self, language: &str, records: Vec<FactRecord>) -> Result<(), LookupError> {
        for record in records {
            match record {
                FactRecord::Node(node) => self.insert_node(language, node)?,
                FactRecord::Edge(record) => self.edges.push(StoredEdge {
                    language: language.to_owned(),
                    record,
                }),
                FactRecord::Unresolved(record) => self.unresolved.push(StoredUnresolved {
                    language: language.to_owned(),
                    record,
                }),
            }
        }
        Ok(())
    }

    fn ingest_nodes(&mut self, language: &str, nodes: Vec<NodeRecord>) -> Result<(), LookupError> {
        for node in nodes {
            self.insert_node(language, node)?;
        }
        Ok(())
    }

    fn insert_node(&mut self, language: &str, node: NodeRecord) -> Result<(), LookupError> {
        let candidate = LookupNode {
            language: language.to_owned(),
            node,
        };
        if let Some(existing) = self.nodes.get(&candidate.node.id) {
            if existing != &candidate {
                return Err(operation(format!(
                    "conflicting lookup node identity {}",
                    candidate.node.id
                ))
                .into());
            }
            return Ok(());
        }
        self.nodes.insert(candidate.node.id.clone(), candidate);
        Ok(())
    }
}

fn resolve_snapshot(
    store: &Store,
    snapshot: &str,
) -> Result<(String, SnapshotManifest), StorageError> {
    if snapshot.trim().is_empty() || snapshot.eq_ignore_ascii_case("CURRENT") {
        return store.current();
    }
    store
        .load_snapshot(snapshot)
        .map(|manifest| (snapshot.to_owned(), manifest))
}

fn validate_metadata(
    object: &FactObject,
    entry: &LanguageEntry,
    expected_file: Option<(&str, &str)>,
) -> Result<(), StorageError> {
    if object.language != entry.language
        || object.adapter_version != entry.adapter_version
        || object.schema_version != entry.schema_version
        || object.analysis_config_id != entry.analysis_config_id
    {
        return Err(operation(format!(
            "lookup object metadata does not match {:?} manifest",
            entry.language
        )));
    }
    match expected_file {
        Some((path, content_id))
            if object.owner != path || object.source_content_id != content_id =>
        {
            Err(operation(format!(
                "lookup object metadata does not match manifest file {path:?}"
            )))
        }
        None if !object.owner.is_empty() || !object.source_content_id.is_empty() => Err(operation(
            "lookup shared object unexpectedly has file ownership",
        )),
        _ => Ok(()),
    }
}

fn operation(message: impl Into<String>) -> StorageError {
    StorageError::Operation(message.into())
}
