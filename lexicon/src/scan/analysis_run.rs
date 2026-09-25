use std::path::Path;

use crate::{AdapterHost, AdapterMode, AdapterRequest, Analysis};

use super::{AnalysisPlan, ExecutionPlan, ScanExecutionError};

pub(crate) fn run_analysis(
    host: &AdapterHost,
    request: &AdapterRequest,
) -> Result<Analysis, ScanExecutionError> {
    host.analyze(request).map_err(ScanExecutionError::from)
}

pub(crate) fn retry_full(
    host: &AdapterHost,
    request: &AdapterRequest,
    source_root: &Path,
    scoped_error: Option<&ScanExecutionError>,
) -> Result<Analysis, ScanExecutionError> {
    let mut full = request.clone();
    full.repository = source_root.to_path_buf();
    full.mode = AdapterMode::Full;
    full.changed_files.clear();
    full.removed_files.clear();

    let analysis = run_analysis(host, &full).map_err(|error| {
        if let Some(scoped) = scoped_error {
            ScanExecutionError::new(format!(
                "scoped {} analysis failed: {scoped}; full retry failed: {error}",
                request.language
            ))
        } else {
            error
        }
    })?;
    if analysis.is_incremental() {
        return Err(ScanExecutionError::new(format!(
            "full {} retry emitted incremental output",
            request.language
        )));
    }
    Ok(analysis)
}

pub(crate) fn request_for_plan(
    plan: &AnalysisPlan,
    execution: &ExecutionPlan,
    repository: std::path::PathBuf,
) -> AdapterRequest {
    AdapterRequest {
        language: plan.language.clone(),
        mode: if plan.full {
            AdapterMode::Full
        } else {
            AdapterMode::Incremental
        },
        repository,
        changed_files: plan.changed_files.clone(),
        removed_files: plan.removed_files.clone(),
        workers: execution.active_workers,
        shards: execution.logical_shards,
        merge_fan_in: execution.merge_fan_in,
    }
}
