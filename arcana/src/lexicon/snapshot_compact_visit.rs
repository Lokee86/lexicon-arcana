use std::fs;
use std::path::Path;

use super::LexiconSnapshotError;
use super::binary::{is_binary_object, parse_binary_object_selected};
use super::binary_v2::MAGIC;
use super::binary_v2_stream::{RelationRef, visit_nodes, visit_relations};
use super::format::{LanguageEntry, Manifest};
use super::object::{FactObject, FactRecord, RecordSelection, parse_json_object};
use super::snapshot::{OBJECT_DOMAIN, validate_object};
use super::snapshot_support::{hex_id, validate_id, verify_content};
use super::stream_compact::CompactPass;
use super::stream_compact_legacy;
use crate::repository::normalize_repository_path;

pub(super) fn visit_node_pass(
    storage: &Path,
    manifest: &Manifest,
    pass: &mut CompactPass,
) -> Result<bool, LexiconSnapshotError> {
    let mut direct_v2 = true;
    for language in &manifest.languages {
        if let Some(object_id) = &language.shared_object_id {
            direct_v2 &= visit_node_object(storage, object_id, language, None, None, pass)?;
        }
        for file in &language.files {
            let path = normalize_file_path(&file.path)?;
            direct_v2 &= visit_node_object(
                storage,
                &file.object_id,
                language,
                Some(&path),
                Some(&file.content_id),
                pass,
            )?;
        }
    }
    Ok(direct_v2)
}

pub(super) fn visit_relation_pass(
    storage: &Path,
    manifest: &Manifest,
    pass: &mut CompactPass,
) -> Result<(), LexiconSnapshotError> {
    for language in &manifest.languages {
        if let Some(object_id) = &language.shared_object_id {
            visit_relation_object(storage, object_id, pass)?;
        }
        for file in &language.files {
            visit_relation_object(storage, &file.object_id, pass)?;
        }
    }
    Ok(())
}

fn visit_node_object(
    storage: &Path,
    id: &str,
    language: &LanguageEntry,
    owner: Option<&str>,
    content_id: Option<&str>,
    pass: &mut CompactPass,
) -> Result<bool, LexiconSnapshotError> {
    let bytes = read_object(storage, id)?;
    if bytes.starts_with(MAGIC) {
        let (object, _) = visit_nodes(&bytes, |record| pass.ingest_node(record))?;
        validate_object(&object, language, owner, content_id)?;
        return Ok(true);
    }

    let object = parse_legacy(&bytes, RecordSelection::Nodes)?;
    validate_object(&object, language, owner, content_id)?;
    for record in object.records {
        if let FactRecord::Node(record) = record {
            stream_compact_legacy::ingest_node(pass, record)?;
        }
    }
    Ok(false)
}

fn visit_relation_object(
    storage: &Path,
    id: &str,
    pass: &mut CompactPass,
) -> Result<(), LexiconSnapshotError> {
    let bytes = read_object(storage, id)?;
    if bytes.starts_with(MAGIC) {
        visit_relations(&bytes, |record| match record {
            RelationRef::Edge(record) => pass.ingest_edge(record),
            RelationRef::Unresolved(record) => pass.ingest_unresolved(record),
        })?;
        return Ok(());
    }

    for record in parse_legacy(&bytes, RecordSelection::Relations)?.records {
        match record {
            FactRecord::Edge(record) => stream_compact_legacy::ingest_edge(pass, record)?,
            FactRecord::Unresolved(record) => {
                stream_compact_legacy::ingest_unresolved(pass, record)?
            }
            FactRecord::Node(_) => {}
        }
    }
    Ok(())
}

fn parse_legacy(
    bytes: &[u8],
    selection: RecordSelection,
) -> Result<FactObject, LexiconSnapshotError> {
    if is_binary_object(bytes) {
        return parse_binary_object_selected(bytes, selection).map(|(object, _)| object);
    }
    let mut object = parse_json_object(bytes)?;
    object.records.retain(|record| selection.includes(record));
    Ok(object)
}

fn read_object(storage: &Path, id: &str) -> Result<Vec<u8>, LexiconSnapshotError> {
    validate_id(id)?;
    let path = storage
        .join("objects")
        .join(&hex_id(id)[..2])
        .join(&hex_id(id)[2..]);
    let bytes = fs::read(path)?;
    let canonical = if is_binary_object(&bytes) {
        bytes
    } else {
        bytes.trim_ascii().to_vec()
    };
    verify_content(&canonical, id, OBJECT_DOMAIN, "fact object")?;
    Ok(canonical)
}

fn normalize_file_path(path: &str) -> Result<String, LexiconSnapshotError> {
    normalize_repository_path(path).map_err(|_| LexiconSnapshotError::InvalidPath {
        field: "file",
        path: path.to_owned(),
    })
}
