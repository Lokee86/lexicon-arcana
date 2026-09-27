use std::fs;
use std::path::Path;

use super::binary_v2::MAGIC;
use super::binary_v2_stream::{RelationRef, visit_nodes, visit_relations};
use super::format::Manifest;
use super::snapshot::{OBJECT_DOMAIN, read_manifest, validate_object};
use super::snapshot_support::{hex_id, storage_root, validate_id, verify_content};
use super::stream_compact::CompactPass;
use super::{LexiconSnapshotError, LexiconSnapshotMetadata};
use crate::repository::normalize_repository_path;
use crate::repository_store::CompactRepositoryBuild;

pub(crate) struct CompactLexiconSnapshot {
    pub(crate) metadata: LexiconSnapshotMetadata,
    pub(crate) repository: CompactRepositoryBuild,
    pub(crate) compatibility_warnings: Vec<String>,
    pub(crate) direct_v2: bool,
}

pub(crate) fn load_compact(
    root: impl AsRef<Path>,
    id: &str,
) -> Result<CompactLexiconSnapshot, LexiconSnapshotError> {
    let storage = storage_root(root.as_ref());
    let (manifest, metadata) = read_manifest(&storage, id)?;
    let mut pass = CompactPass::new();

    if !visit_nodes_pass(&storage, &manifest, &mut pass)? {
        return legacy_fallback(root, id);
    }
    pass.finish_node_pass();
    visit_relations_pass(&storage, &manifest, &mut pass)?;
    let (repository, compatibility_warnings) = pass.finish()?;
    Ok(CompactLexiconSnapshot {
        metadata,
        repository,
        compatibility_warnings,
        direct_v2: true,
    })
}

fn visit_nodes_pass(
    storage: &Path,
    manifest: &Manifest,
    pass: &mut CompactPass,
) -> Result<bool, LexiconSnapshotError> {
    for language in &manifest.languages {
        if let Some(object_id) = &language.shared_object_id {
            let Some(bytes) = read_v2_object(storage, object_id)? else {
                return Ok(false);
            };
            let (object, _) = visit_nodes(&bytes, |record| pass.ingest_node(record))?;
            validate_object(&object, language, None, None)?;
        }
        for file in &language.files {
            let Some(bytes) = read_v2_object(storage, &file.object_id)? else {
                return Ok(false);
            };
            let (object, _) = visit_nodes(&bytes, |record| pass.ingest_node(record))?;
            let path = normalize_file_path(&file.path)?;
            validate_object(&object, language, Some(&path), Some(&file.content_id))?;
        }
    }
    Ok(true)
}

fn visit_relations_pass(
    storage: &Path,
    manifest: &Manifest,
    pass: &mut CompactPass,
) -> Result<(), LexiconSnapshotError> {
    for language in &manifest.languages {
        if let Some(object_id) = &language.shared_object_id {
            let bytes = require_v2_object(storage, object_id)?;
            visit_relations(&bytes, |record| ingest_relation(pass, record))?;
        }
        for file in &language.files {
            let bytes = require_v2_object(storage, &file.object_id)?;
            visit_relations(&bytes, |record| ingest_relation(pass, record))?;
        }
    }
    Ok(())
}

fn ingest_relation(
    pass: &mut CompactPass,
    record: RelationRef<'_>,
) -> Result<(), LexiconSnapshotError> {
    match record {
        RelationRef::Edge(record) => pass.ingest_edge(record),
        RelationRef::Unresolved(record) => pass.ingest_unresolved(record),
    }
}

fn read_v2_object(storage: &Path, id: &str) -> Result<Option<Vec<u8>>, LexiconSnapshotError> {
    validate_id(id)?;
    let path = storage
        .join("objects")
        .join(&hex_id(id)[..2])
        .join(&hex_id(id)[2..]);
    let bytes = fs::read(path)?;
    if !bytes.starts_with(MAGIC) {
        return Ok(None);
    }
    verify_content(&bytes, id, OBJECT_DOMAIN, "fact object")?;
    Ok(Some(bytes))
}

fn require_v2_object(storage: &Path, id: &str) -> Result<Vec<u8>, LexiconSnapshotError> {
    read_v2_object(storage, id)?.ok_or(LexiconSnapshotError::Malformed(
        "Lexicon object format changed between compact passes",
    ))
}

fn normalize_file_path(path: &str) -> Result<String, LexiconSnapshotError> {
    normalize_repository_path(path).map_err(|_| LexiconSnapshotError::InvalidPath {
        field: "file",
        path: path.to_owned(),
    })
}

fn legacy_fallback(
    root: impl AsRef<Path>,
    id: &str,
) -> Result<CompactLexiconSnapshot, LexiconSnapshotError> {
    let snapshot = super::snapshot::load(root, id)?;
    let repository = CompactRepositoryBuild::from_facts(snapshot.facts())?;
    Ok(CompactLexiconSnapshot {
        metadata: snapshot.metadata().clone(),
        repository,
        compatibility_warnings: snapshot.compatibility_warnings().to_vec(),
        direct_v2: false,
    })
}
