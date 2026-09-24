use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::languages::supports_streaming_output;
use crate::{AdapterError, AdapterHost, AdapterRequest, Analysis};

use super::{AnalysisPlan, ExecutionPlan, ScanExecutionError};

pub(crate) fn run_analysis(
    host: &AdapterHost,
    request: &AdapterRequest,
) -> Result<Analysis, ScanExecutionError> {
    if supports_streaming_output(&request.language) {
        return host
            .run_stream(request, |reader| parse_reader(reader))
            .map_err(ScanExecutionError::from);
    }
    host.run(request)?;
    let text = fs::read_to_string(&request.output)?;
    Analysis::parse(&text).map_err(ScanExecutionError::from)
}

pub(crate) fn retry_full(
    host: &AdapterHost,
    request: &AdapterRequest,
    source_root: &Path,
    scoped_error: Option<&ScanExecutionError>,
) -> Result<Analysis, ScanExecutionError> {
    let mut full = request.clone();
    full.repository = source_root.to_path_buf();
    full.changed_files.clear();
    full.removed_files.clear();
    let _ = fs::remove_file(&full.output);

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
    repository: PathBuf,
    output: PathBuf,
) -> AdapterRequest {
    AdapterRequest {
        language: plan.language.clone(),
        repository,
        output,
        changed_files: plan.changed_files.clone(),
        removed_files: plan.removed_files.clone(),
        workers: execution.active_workers,
        shards: execution.logical_shards,
        merge_fan_in: execution.merge_fan_in,
    }
}

fn parse_reader(reader: &mut dyn Read) -> Result<Analysis, AdapterError> {
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    Analysis::parse(&text).map_err(|error| AdapterError::new(error.to_string()))
}
