use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::languages::{owns_source, supports_partitioned_execution};

use super::AnalysisPlan;

const STANDARD_SHARD_UNITS: usize = 64;
const ENTERPRISE_SHARD_UNITS: usize = 32;
const ENTERPRISE_FILE_THRESHOLD: usize = 2_000;
const ENTERPRISE_BYTES_THRESHOLD: u64 = 128 * 1024 * 1024;
const MAX_LOGICAL_SHARDS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub language: String,
    pub source_files: usize,
    pub source_bytes: u64,
    pub logical_shards: usize,
    pub active_workers: usize,
    pub merge_fan_in: usize,
    pub reserved_weight: usize,
}

pub fn execution_plan(
    source_root: &Path,
    plan: &AnalysisPlan,
) -> Result<ExecutionPlan, std::io::Error> {
    let parallelism = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let worker_limit = std::env::var("LEXICON_MAX_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0);
    execution_plan_with_limits(source_root, plan, parallelism, worker_limit)
}

pub fn execution_plan_with_limits(
    source_root: &Path,
    plan: &AnalysisPlan,
    parallelism: usize,
    worker_limit: Option<usize>,
) -> Result<ExecutionPlan, std::io::Error> {
    let mut result = ExecutionPlan {
        language: plan.language.clone(),
        source_files: 0,
        source_bytes: 0,
        logical_shards: 1,
        active_workers: 1,
        merge_fan_in: 2,
        reserved_weight: 1,
    };
    if !supports_partitioned_execution(&plan.language) {
        return Ok(result);
    }

    let (paths, bytes) = analysis_inventory(source_root, plan)?;
    result.source_files = paths.len();
    result.source_bytes = bytes;
    if paths.len() < 2 {
        return Ok(result);
    }

    result.logical_shards = logical_shard_count(paths.len(), bytes);
    let mut workers = parallelism.max(1);
    if let Some(limit) = worker_limit.filter(|limit| *limit < workers) {
        workers = limit;
    }

    let enterprise =
        paths.len() >= ENTERPRISE_FILE_THRESHOLD || bytes >= ENTERPRISE_BYTES_THRESHOLD;
    let worker_ceiling = if enterprise {
        result.logical_shards
    } else {
        result.logical_shards.div_ceil(2)
    };
    result.active_workers = workers.min(worker_ceiling).max(1);
    result.reserved_weight = result.active_workers;
    result.merge_fan_in = match result.logical_shards {
        value if value > 64 => 8,
        value if value >= 8 => 4,
        _ => 2,
    };
    Ok(result)
}

pub fn logical_shard_count(file_count: usize, source_bytes: u64) -> usize {
    if file_count < 2 {
        return 1;
    }
    let byte_units = source_bytes.div_ceil(64 * 1024) as usize;
    let units = file_count.max(byte_units);
    let shard_units =
        if file_count >= ENTERPRISE_FILE_THRESHOLD || source_bytes >= ENTERPRISE_BYTES_THRESHOLD {
            ENTERPRISE_SHARD_UNITS
        } else {
            STANDARD_SHARD_UNITS
        };
    let target = units.div_ceil(shard_units);
    next_power_of_two(target)
        .min(file_count)
        .min(MAX_LOGICAL_SHARDS)
}

fn analysis_inventory(
    source_root: &Path,
    plan: &AnalysisPlan,
) -> Result<(Vec<String>, u64), std::io::Error> {
    if !plan.full {
        let paths: BTreeSet<String> = plan
            .changed_files
            .iter()
            .chain(&plan.context_files)
            .filter(|path| !path.is_empty())
            .map(|path| path.replace('\\', "/"))
            .collect();
        return stat_inventory(source_root, paths);
    }
    full_inventory(source_root, &plan.language)
}

fn stat_inventory(
    source_root: &Path,
    paths: BTreeSet<String>,
) -> Result<(Vec<String>, u64), std::io::Error> {
    let mut kept = Vec::new();
    let mut bytes = 0;
    for path in paths {
        match fs::metadata(source_root.join(path_from_slash(&path))) {
            Ok(metadata) => {
                kept.push(path);
                bytes += metadata.len();
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok((kept, bytes))
}

fn full_inventory(
    source_root: &Path,
    language: &str,
) -> Result<(Vec<String>, u64), std::io::Error> {
    let mut paths = Vec::new();
    let mut bytes = 0;
    let mut stack = vec![source_root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(source_root)
                .expect("walked path must remain beneath source root");
            let normalized = relative.to_string_lossy().replace('\\', "/");
            if owns_source(language, &normalized) {
                bytes += entry.metadata()?.len();
                paths.push(normalized);
            }
        }
    }
    paths.sort();
    Ok((paths, bytes))
}

fn next_power_of_two(value: usize) -> usize {
    let mut result = 1;
    while result < value && result < MAX_LOGICAL_SHARDS {
        result <<= 1;
    }
    result
}

fn path_from_slash(value: &str) -> PathBuf {
    value.split('/').collect()
}
