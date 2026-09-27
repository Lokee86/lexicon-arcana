use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::config::ANALYSIS_CONFIG_ID;
use crate::{AdapterHost, Analysis, LanguageEntry, SnapshotManifest, Store, build_analysis_scope};

use super::analysis_run::{request_for_plan, retry_full, run_analysis};
use super::sources::{language_present, language_sources, selected_sources};
use super::{
    AnalysisPlan, ExecutionBudget, LanguageResult, ScanExecutionError, assemble_manifest,
    execution_plan,
};

pub fn execute_analysis_plans(
    store: &Store,
    host: &AdapterHost,
    source_root: &Path,
    temporary_root: &Path,
    manifest: SnapshotManifest,
    plans: &[AnalysisPlan],
) -> Result<SnapshotManifest, ScanExecutionError> {
    fs::create_dir_all(temporary_root)?;
    let prepared = plans
        .iter()
        .map(|plan| execution_plan(source_root, plan).map(|execution| (plan.clone(), execution)))
        .collect::<Result<Vec<_>, _>>()?;
    let capacity = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let budget = ExecutionBudget::new(capacity);

    let results = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(prepared.len());
        for (plan, execution) in prepared {
            let previous = &manifest;
            let budget = &budget;
            handles.push(scope.spawn(move || {
                let _permit = budget.acquire(execution.reserved_weight);
                execute_plan(
                    store,
                    host,
                    source_root,
                    temporary_root,
                    previous,
                    &plan,
                    &execution,
                )
            }));
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| ScanExecutionError::new("analysis worker panicked"))?
            })
            .collect::<Result<Vec<_>, _>>()
    })?;

    assemble_manifest(manifest, results).map_err(ScanExecutionError::from)
}

fn execute_plan(
    store: &Store,
    host: &AdapterHost,
    source_root: &Path,
    temporary_root: &Path,
    manifest: &SnapshotManifest,
    plan: &AnalysisPlan,
    execution: &super::ExecutionPlan,
) -> Result<LanguageResult, ScanExecutionError> {
    if !plan.known_present && !language_present(source_root, &plan.language)? {
        return Ok(LanguageResult {
            language: plan.language.clone(),
            entry: None,
        });
    }

    let repository = if plan.full {
        source_root.to_path_buf()
    } else {
        let mut context: BTreeSet<String> = plan.context_files.iter().cloned().collect();
        if plan.language == "python" && !plan.changed_files.is_empty() {
            context.extend(super::python_scope::changed_context(
                source_root,
                &plan.changed_files,
            )?);
        }
        let context = context.into_iter().collect::<Vec<_>>();
        build_analysis_scope(
            source_root,
            &temporary_root.join("scopes"),
            &plan.language,
            &context,
        )?
    };
    let request = request_for_plan(plan, execution, repository);

    let entry = if plan.full {
        let analysis = run_analysis(host, &request)?;
        apply_full(store, host, source_root, &plan.language, &analysis)?
    } else {
        execute_incremental(store, host, source_root, manifest, plan, &request)?
    };
    Ok(LanguageResult {
        language: plan.language.clone(),
        entry: Some(entry),
    })
}

fn execute_incremental(
    store: &Store,
    host: &AdapterHost,
    source_root: &Path,
    manifest: &SnapshotManifest,
    plan: &AnalysisPlan,
    request: &crate::AdapterRequest,
) -> Result<LanguageEntry, ScanExecutionError> {
    let analysis = match run_analysis(host, request) {
        Ok(analysis) => analysis,
        Err(scoped) => {
            let full = retry_full(host, request, source_root, Some(&scoped))?;
            return apply_full(store, host, source_root, &plan.language, &full);
        }
    };
    if !analysis.is_incremental() {
        let full = retry_full(host, request, source_root, None)?;
        return apply_full(store, host, source_root, &plan.language, &full);
    }
    if store.requires_full_analysis(&plan.language, &plan.changed_files, &analysis)? {
        let full = retry_full(host, request, source_root, None)?;
        return apply_full(store, host, source_root, &plan.language, &full);
    }
    let Some(previous) = manifest.language(&plan.language) else {
        let full = retry_full(host, request, source_root, None)?;
        return apply_full(store, host, source_root, &plan.language, &full);
    };
    let fingerprint = host.fingerprint(&plan.language)?;
    let sources = selected_sources(source_root, &plan.changed_files)?;
    let storage_started = if plan.language == "go" {
        crate::perf::start()
    } else {
        None
    };
    let replace_shared = analysis.header.shared_complete.unwrap_or(false);
    let result = store.build_incremental_language(
        previous,
        &analysis,
        &sources,
        ANALYSIS_CONFIG_ID,
        &fingerprint,
        &plan.changed_files,
        &plan.removed_files,
        replace_shared,
    );
    if let Some(storage_started) = storage_started {
        crate::perf::emit(
            "go.final_serialization_storage",
            storage_started.elapsed(),
            &[("final_fact_count", analysis.records.len() as u64)],
        );
    }
    result.map_err(ScanExecutionError::from)
}

fn apply_full(
    store: &Store,
    host: &AdapterHost,
    source_root: &Path,
    language: &str,
    analysis: &Analysis,
) -> Result<LanguageEntry, ScanExecutionError> {
    let fingerprint = host.fingerprint(language)?;
    let sources = language_sources(source_root, language)?;
    let storage_started = if language == "go" {
        crate::perf::start()
    } else {
        None
    };
    let result = store.build_full_language(
        analysis,
        &sources,
        language,
        ANALYSIS_CONFIG_ID,
        &fingerprint,
    );
    if let Some(storage_started) = storage_started {
        crate::perf::emit(
            "go.final_serialization_storage",
            storage_started.elapsed(),
            &[("final_fact_count", analysis.records.len() as u64)],
        );
    }
    result.map_err(ScanExecutionError::from)
}
