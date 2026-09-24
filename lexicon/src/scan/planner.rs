use std::collections::{BTreeMap, BTreeSet};

use crate::languages::{for_path, language_enabled, owns_source};
use crate::{SnapshotManifest, StorageError, Store};

use super::{AnalysisPlan, Change, PlanningInput, ScanPlan};

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
    add_change_plans(&mut plans, &input.changes, &input.enabled_languages);

    let mut analyses = Vec::with_capacity(plans.len());
    for (_, mut plan) in plans {
        if !plan.full {
            let roots = unique_sorted(&plan.changed_files);
            match store.incremental_scope(&plan.language, &roots) {
                Ok(scope) if !scope.full_required => {
                    plan.changed_files = scope.emit;
                    plan.removed_files = Vec::new();
                    plan.context_files = scope.context;
                }
                Ok(_) | Err(_) => plan.full = true,
            }
        }
        if plan.full {
            plan.changed_files.clear();
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

fn add_change_plans(
    plans: &mut BTreeMap<String, AnalysisPlan>,
    changes: &[Change],
    enabled: &[String],
) {
    for change in changes {
        for path in [&change.new, &change.old] {
            if path.is_empty() {
                continue;
            }
            for language in for_path(path) {
                if !language_enabled(&language, enabled) {
                    continue;
                }
                let plan = plans
                    .entry(language.clone())
                    .or_insert_with(|| incremental_plan(language.clone()));
                if structural_change(change, &language, path) {
                    plan.full = true;
                    continue;
                }
                if !change.new.is_empty() && owns_source(&language, &change.new) {
                    plan.changed_files.push(change.new.clone());
                }
            }
        }
    }
}

fn structural_change(change: &Change, language: &str, path: &str) -> bool {
    let status = change.status.trim();
    status.as_bytes().first().copied() != Some(b'M') || !owns_source(language, path)
}

fn full_plan(language: String) -> AnalysisPlan {
    AnalysisPlan {
        language,
        full: true,
        known_present: false,
        changed_files: Vec::new(),
        removed_files: Vec::new(),
        context_files: Vec::new(),
    }
}

fn incremental_plan(language: String) -> AnalysisPlan {
    AnalysisPlan {
        language,
        full: false,
        known_present: false,
        changed_files: Vec::new(),
        removed_files: Vec::new(),
        context_files: Vec::new(),
    }
}

fn unique_sorted(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| !path.is_empty())
        .map(|path| path.replace('\\', "/"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
