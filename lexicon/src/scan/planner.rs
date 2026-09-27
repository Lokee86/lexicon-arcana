use std::collections::{BTreeMap, BTreeSet};

use crate::languages::language_enabled;
use crate::{SnapshotManifest, StorageError, Store};

use super::planner_changes::add_change_plans;
use super::{AnalysisPlan, PlanningInput, ScanPlan};

pub fn plan_scan(
    store: &Store,
    manifest: &SnapshotManifest,
    input: &PlanningInput,
) -> Result<ScanPlan, StorageError> {
    let (manifest, pruned_disabled_languages) =
        prune_disabled_languages(manifest.clone(), &input.enabled_languages);
    let mut plans = BTreeMap::<String, AnalysisPlan>::new();

    for language in snapshot_drift_languages(&manifest, input) {
        plans.insert(language.clone(), full_plan(language));
    }
    for language in adapter_drift_languages(&manifest, input)? {
        plans.insert(language.clone(), full_plan(language));
    }
    add_change_plans(&mut plans, input);

    let mut analyses = Vec::with_capacity(plans.len());
    for (_, mut plan) in plans {
        if !plan.full {
            let removed = unique_sorted(&plan.removed_files);
            let mut roots = plan.changed_files.clone();
            roots.extend(removed.iter().cloned());
            let roots = unique_sorted(&roots);
            let added = unique_sorted(&plan.added_files);
            match store.incremental_scope_with_additions(&plan.language, &roots, &added) {
                Ok(scope) if !scope.full_required => {
                    let mut changed = scope.emit;
                    changed.extend(added.iter().cloned());
                    plan.changed_files = without_paths(unique_sorted(&changed), &removed);
                    plan.added_files = added.clone();
                    plan.removed_files = removed.clone();
                    let mut context = scope.context;
                    context.extend(added);
                    plan.context_files = without_paths(unique_sorted(&context), &removed);
                }
                Ok(_) | Err(_) => plan.full = true,
            }
        }
        if plan.full {
            plan.changed_files.clear();
            plan.added_files.clear();
            plan.removed_files.clear();
            plan.context_files.clear();
        }
        analyses.push(plan);
    }

    Ok(ScanPlan {
        manifest,
        analyses,
        pruned_disabled_languages,
    })
}

pub(super) fn prune_disabled_languages(
    mut manifest: SnapshotManifest,
    enabled: &[String],
) -> (SnapshotManifest, bool) {
    let languages: Vec<String> = manifest
        .languages
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.language.clone())
        .collect();
    let mut pruned = false;
    for language in languages {
        if language == "interstack" || language_enabled(&language, enabled) {
            continue;
        }
        manifest = manifest.without_language(&language);
        pruned = true;
    }
    (manifest, pruned)
}

fn snapshot_drift_languages(manifest: &SnapshotManifest, input: &PlanningInput) -> Vec<String> {
    let required: BTreeSet<String> = input
        .present_languages
        .iter()
        .filter(|language| language_enabled(language, &input.enabled_languages))
        .cloned()
        .collect();
    let present: BTreeSet<String> = manifest
        .languages
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|entry| entry.language != "interstack")
        .map(|entry| entry.language.clone())
        .collect();

    required.symmetric_difference(&present).cloned().collect()
}

fn adapter_drift_languages(
    manifest: &SnapshotManifest,
    input: &PlanningInput,
) -> Result<Vec<String>, StorageError> {
    let Some(fingerprints) = input.adapter_fingerprints.as_ref() else {
        return Ok(Vec::new());
    };
    let mut drift = Vec::new();
    for entry in manifest.languages.as_deref().unwrap_or_default() {
        if entry.language == "interstack" {
            continue;
        }
        let fingerprint = fingerprints.get(&entry.language).ok_or_else(|| {
            StorageError::Materialization(format!(
                "missing adapter fingerprint for {:?}",
                entry.language
            ))
        })?;
        if fingerprint != &entry.adapter_fingerprint {
            drift.push(entry.language.clone());
        }
    }
    drift.sort();
    Ok(drift)
}

fn full_plan(language: String) -> AnalysisPlan {
    AnalysisPlan {
        language,
        full: true,
        known_present: false,
        changed_files: Vec::new(),
        added_files: Vec::new(),
        removed_files: Vec::new(),
        context_files: Vec::new(),
    }
}

pub(super) fn incremental_plan(language: String) -> AnalysisPlan {
    AnalysisPlan {
        language,
        full: false,
        known_present: false,
        changed_files: Vec::new(),
        added_files: Vec::new(),
        removed_files: Vec::new(),
        context_files: Vec::new(),
    }
}

pub(super) fn unique_sorted(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| !path.is_empty())
        .map(|path| path.replace('\\', "/"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn without_paths(paths: Vec<String>, removed: &[String]) -> Vec<String> {
    if removed.is_empty() {
        return paths;
    }
    let removed: BTreeSet<&str> = removed.iter().map(String::as_str).collect();
    paths
        .into_iter()
        .filter(|path| !removed.contains(path.as_str()))
        .collect()
}
