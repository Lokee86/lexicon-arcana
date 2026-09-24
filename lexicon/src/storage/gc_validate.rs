use std::collections::BTreeSet;

use super::export::operation;
use super::store::validate_storage_id;
use super::{GcPlan, SnapshotManifest, StorageError};

pub(super) fn add_manifest_objects(
    objects: &mut BTreeSet<String>,
    manifest: &SnapshotManifest,
) -> Result<(), StorageError> {
    for language in manifest.languages.as_deref().unwrap_or_default() {
        if !language.shared_object_id.is_empty() {
            validate_storage_id(&language.shared_object_id).map_err(|_| {
                operation(format!(
                    "invalid shared_object_id {:?}",
                    language.shared_object_id
                ))
            })?;
            objects.insert(language.shared_object_id.clone());
        }
        for file in language.files.as_deref().unwrap_or_default() {
            validate_storage_id(&file.object_id)
                .map_err(|_| operation(format!("invalid object_id {:?}", file.object_id)))?;
            objects.insert(file.object_id.clone());
        }
    }
    Ok(())
}

pub(super) fn canonical_plan(mut plan: GcPlan) -> Result<GcPlan, StorageError> {
    validate_storage_id(&plan.current_snapshot).map_err(|_| {
        operation(format!(
            "invalid Lexicon GC current snapshot {:?}",
            plan.current_snapshot
        ))
    })?;
    validate_ids("snapshot", &plan.preserved_snapshots)?;
    validate_ids("snapshot", &plan.delete_snapshots)?;
    validate_ids("object", &plan.preserved_objects)?;
    validate_ids("object", &plan.delete_objects)?;

    let preserved_snapshots = plan
        .preserved_snapshots
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if !preserved_snapshots.contains(&plan.current_snapshot) {
        return Err(operation(format!(
            "Lexicon GC plan does not preserve CURRENT snapshot {}",
            plan.current_snapshot
        )));
    }
    reject_overlap("snapshot", &preserved_snapshots, &plan.delete_snapshots)?;
    reject_overlap(
        "object",
        &plan.preserved_objects.iter().cloned().collect(),
        &plan.delete_objects,
    )?;

    plan.preserved_snapshots.sort();
    plan.delete_snapshots.sort();
    plan.preserved_objects.sort();
    plan.delete_objects.sort();
    Ok(plan)
}

fn validate_ids(kind: &str, ids: &[String]) -> Result<(), StorageError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        validate_storage_id(id)
            .map_err(|_| operation(format!("invalid Lexicon GC {kind} ID {id:?}")))?;
        if !seen.insert(id) {
            return Err(operation(format!("duplicate Lexicon GC {kind} ID {id}")));
        }
    }
    Ok(())
}

fn reject_overlap(
    kind: &str,
    preserved: &BTreeSet<String>,
    deleted: &[String],
) -> Result<(), StorageError> {
    if let Some(id) = deleted.iter().find(|id| preserved.contains(*id)) {
        return Err(operation(format!(
            "Lexicon GC plan deletes preserved {kind} {id}"
        )));
    }
    Ok(())
}
