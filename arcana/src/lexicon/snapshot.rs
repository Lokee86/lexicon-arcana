use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use super::binary::{is_binary_object, parse_binary_object_selected};
use super::format::{LanguageEntry, Manifest};
use super::identity::LexiconIdentity;
use super::object::{FactObject, RecordCounts, RecordSelection, parse_json_object};
use super::snapshot_support::{
    hex_id, read_verified_json, storage_root, validate_id, verify_content,
};
use super::stream_records::NodePass;
use super::{
    FACT_SCHEMA_VERSION, LexiconSnapshot, LexiconSnapshotError, LexiconSnapshotMetadata,
    OBJECT_VERSION, SNAPSHOT_VERSION,
};
use crate::repository::normalize_repository_path;

const SNAPSHOT_DOMAIN: &str = "lexicon:snapshot:v1\0";
const OBJECT_DOMAIN: &str = "lexicon:fact-object:v1\0";

pub fn current(root: impl AsRef<Path>) -> Result<LexiconSnapshot, LexiconSnapshotError> {
    let root = root.as_ref();
    let id = current_id(root)?;
    load(root, &id)
}

pub fn current_metadata(
    root: impl AsRef<Path>,
) -> Result<LexiconSnapshotMetadata, LexiconSnapshotError> {
    let root = root.as_ref();
    let id = current_id(root)?;
    load_metadata(root, &id)
}

pub fn load_metadata(
    root: impl AsRef<Path>,
    id: &str,
) -> Result<LexiconSnapshotMetadata, LexiconSnapshotError> {
    let storage = storage_root(root.as_ref());
    let (_, metadata) = read_manifest(&storage, id)?;
    Ok(metadata)
}

pub fn load(root: impl AsRef<Path>, id: &str) -> Result<LexiconSnapshot, LexiconSnapshotError> {
    let storage = storage_root(root.as_ref());
    let (manifest, metadata) = read_manifest(&storage, id)?;

    let node_started = Instant::now();
    let mut nodes = NodePass::new();
    let node_visit = visit_objects(
        &storage,
        &manifest,
        RecordSelection::Nodes,
        true,
        |object, counts| {
            nodes.ingest(object.records, counts);
        },
    )?;
    profile_pass("node-pass", node_started.elapsed(), node_visit);

    let mut relations = nodes.finish()?;
    let relation_started = Instant::now();
    let relation_visit = visit_objects(
        &storage,
        &manifest,
        RecordSelection::Relations,
        false,
        |object, _| {
            relations.ingest(object.records);
        },
    )?;
    profile_pass("relation-pass", relation_started.elapsed(), relation_visit);

    let finish_started = Instant::now();
    let (facts, compatibility_warnings) = relations.finish()?;
    if profile_enabled() {
        eprintln!(
            "arcana sync profile: phase=repository-facts elapsed_ms={:.3} nodes={}/{} edges={}/{} unresolved={}/{}",
            finish_started.elapsed().as_secs_f64() * 1000.0,
            facts.nodes.len(),
            facts.nodes.capacity(),
            facts.edges.len(),
            facts.edges.capacity(),
            facts.unresolved.len(),
            facts.unresolved.capacity(),
        );
    }

    Ok(LexiconSnapshot {
        metadata,
        facts,
        compatibility_warnings,
    })
}

fn current_id(root: &Path) -> Result<String, LexiconSnapshotError> {
    let storage = storage_root(root);
    let current = fs::read(storage.join("CURRENT"))?;
    let text = std::str::from_utf8(&current).map_err(|_| LexiconSnapshotError::InvalidCurrent)?;
    let id = text
        .strip_suffix('\n')
        .filter(|value| !value.is_empty() && !value.chars().any(char::is_whitespace))
        .ok_or(LexiconSnapshotError::InvalidCurrent)?;
    validate_id(id)?;
    Ok(id.to_owned())
}

fn read_manifest(
    storage: &Path,
    id: &str,
) -> Result<(Manifest, LexiconSnapshotMetadata), LexiconSnapshotError> {
    validate_id(id)?;
    let manifest_bytes = read_verified_json(
        &storage
            .join("snapshots")
            .join(format!("{}.json", hex_id(id))),
        id,
        SNAPSHOT_DOMAIN,
        "snapshot manifest",
    )?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.version != SNAPSHOT_VERSION {
        return Err(LexiconSnapshotError::UnsupportedSnapshotVersion(
            manifest.version,
        ));
    }
    if manifest.state_commit.is_empty() {
        return Err(LexiconSnapshotError::Malformed("manifest metadata"));
    }

    let mut files = BTreeMap::new();
    let mut shared_objects = BTreeMap::new();
    let mut previous_language = None;
    for language in &manifest.languages {
        validate_language(language)?;
        if previous_language
            .as_deref()
            .is_some_and(|previous| previous >= language.language.as_str())
        {
            return Err(LexiconSnapshotError::Malformed("language ordering"));
        }
        previous_language = Some(language.language.clone());

        if let Some(object_id) = &language.shared_object_id {
            validate_id(object_id)?;
        }
        shared_objects.insert(language.language.clone(), language.shared_object_id.clone());

        let mut previous_path = None;
        for file in &language.files {
            if previous_path
                .as_deref()
                .is_some_and(|previous| previous >= file.path.as_str())
            {
                return Err(LexiconSnapshotError::Malformed("file ordering"));
            }
            previous_path = Some(file.path.clone());
            let path = normalize_path("file", &file.path)?;
            validate_id(&file.content_id)?;
            validate_id(&file.object_id)?;
            if file.language != language.language
                || files
                    .insert((language.language.clone(), path), file.object_id.clone())
                    .is_some()
            {
                return Err(LexiconSnapshotError::MetadataMismatch("file entry"));
            }
        }
    }

    Ok((
        manifest,
        LexiconSnapshotMetadata {
            id: id.to_owned(),
            files,
            shared_objects,
        },
    ))
}

fn visit_objects(
    storage: &Path,
    manifest: &Manifest,
    selection: RecordSelection,
    validate_metadata: bool,
    mut visit: impl FnMut(FactObject, RecordCounts),
) -> Result<VisitMetrics, LexiconSnapshotError> {
    let mut metrics = VisitMetrics::default();
    for language in &manifest.languages {
        if let Some(object_id) = &language.shared_object_id {
            let (object, counts, decode_elapsed) = read_object(storage, object_id, selection)?;
            metrics.objects += 1;
            metrics.decode_elapsed += decode_elapsed;
            if validate_metadata {
                validate_object(&object, language, None, None)?;
            }
            visit(object, counts);
        }
        for file in &language.files {
            let (object, counts, decode_elapsed) =
                read_object(storage, &file.object_id, selection)?;
            metrics.objects += 1;
            metrics.decode_elapsed += decode_elapsed;
            if validate_metadata {
                let path = normalize_path("file", &file.path)?;
                validate_object(&object, language, Some(&path), Some(&file.content_id))?;
            }
            visit(object, counts);
        }
    }
    Ok(metrics)
}

fn validate_language(language: &LanguageEntry) -> Result<(), LexiconSnapshotError> {
    if language.language.is_empty()
        || language.adapter_version.is_empty()
        || language.repository.is_empty()
        || language.analysis_config_id.is_empty()
    {
        return Err(LexiconSnapshotError::Malformed("language metadata"));
    }
    if language.schema_version != FACT_SCHEMA_VERSION {
        return Err(LexiconSnapshotError::UnsupportedSchemaVersion(
            language.schema_version,
        ));
    }
    if let Some(fingerprint) = &language.adapter_fingerprint {
        validate_id(fingerprint)?;
    }
    validate_id(&language.analysis_config_id)
}

fn read_object(
    storage: &Path,
    id: &str,
    selection: RecordSelection,
) -> Result<(FactObject, RecordCounts, Duration), LexiconSnapshotError> {
    validate_id(id)?;
    let path = storage
        .join("objects")
        .join(&hex_id(id)[..2])
        .join(&hex_id(id)[2..]);
    let bytes = fs::read(path)?;
    let canonical = if is_binary_object(&bytes) {
        bytes.as_slice()
    } else {
        bytes.trim_ascii()
    };
    verify_content(canonical, id, OBJECT_DOMAIN, "fact object")?;

    let started = Instant::now();
    let (object, counts) = if is_binary_object(canonical) {
        parse_binary_object_selected(canonical, selection)?
    } else {
        let mut object = parse_json_object(canonical)?;
        let counts = RecordCounts::from_records(&object.records);
        if selection != RecordSelection::All {
            object.records.retain(|record| selection.includes(record));
        }
        (object, counts)
    };
    Ok((object, counts, started.elapsed()))
}

fn validate_object(
    object: &FactObject,
    language: &LanguageEntry,
    owner: Option<&str>,
    content_id: Option<&str>,
) -> Result<(), LexiconSnapshotError> {
    if object.version != OBJECT_VERSION {
        return Err(LexiconSnapshotError::UnsupportedObjectVersion(
            object.version,
        ));
    }
    let analysis_config_id = LexiconIdentity::parse(&language.analysis_config_id)?;
    if object.language != language.language
        || object.adapter_version != language.adapter_version
        || object.schema_version != language.schema_version
        || object.analysis_config_id != analysis_config_id
    {
        return Err(LexiconSnapshotError::MetadataMismatch("fact object"));
    }
    match (owner, content_id) {
        (Some(owner), Some(content_id)) => {
            let content_id = LexiconIdentity::parse(content_id)?;
            if object.owner.as_deref() != Some(owner)
                || object.source_content_id != Some(content_id)
            {
                return Err(LexiconSnapshotError::MetadataMismatch("file fact object"));
            }
        }
        (None, None) if object.owner.is_none() && object.source_content_id.is_none() => {}
        _ => {
            return Err(LexiconSnapshotError::MetadataMismatch("shared fact object"));
        }
    }
    Ok(())
}

fn normalize_path(field: &'static str, path: &str) -> Result<String, LexiconSnapshotError> {
    normalize_repository_path(path).map_err(|_| LexiconSnapshotError::InvalidPath {
        field,
        path: path.to_owned(),
    })
}

#[derive(Clone, Copy, Debug, Default)]
struct VisitMetrics {
    objects: usize,
    decode_elapsed: Duration,
}

fn profile_enabled() -> bool {
    std::env::var_os("ARCANA_SYNC_PROFILE").is_some()
}

fn profile_pass(phase: &str, elapsed: Duration, metrics: VisitMetrics) {
    if profile_enabled() {
        eprintln!(
            "arcana sync profile: phase={phase} elapsed_ms={:.3} object_decode_ms={:.3} objects={}",
            elapsed.as_secs_f64() * 1000.0,
            metrics.decode_elapsed.as_secs_f64() * 1000.0,
            metrics.objects,
        );
    }
}
