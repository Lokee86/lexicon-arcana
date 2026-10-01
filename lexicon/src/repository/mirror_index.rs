use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

const MAX_HASH_WORKERS: usize = 16;

pub(crate) fn unchanged_files(
    mirror_root: &Path,
    source_root: &Path,
    desired: &BTreeMap<PathBuf, PathBuf>,
) -> Option<BTreeSet<PathBuf>> {
    let state_root = mirror_root.parent()?;
    let clean = mirror_clean(state_root);
    if crate::perf::enabled() {
        crate::perf::emit(
            "scan.mirror_index_state",
            std::time::Duration::ZERO,
            &[
                ("mirror_clean", u64::from(matches!(clean, Some(true)))),
                ("mirror_dirty", u64::from(matches!(clean, Some(false)))),
                ("mirror_status_failed", u64::from(clean.is_none())),
            ],
        );
    }
    if !clean? {
        return None;
    }
    let index = source_index(state_root);
    if index.is_none() && crate::perf::enabled() {
        crate::perf::emit(
            "scan.mirror_index_fallback",
            std::time::Duration::ZERO,
            &[("source_index_failed", 1), ("source_hash_failed", 0)],
        );
    }
    let index = index?;
    let paths = desired.keys().cloned().collect::<Vec<_>>();
    if paths.iter().any(|path| {
        let value = path.to_string_lossy();
        value.contains('\n') || value.contains('\r')
    }) {
        return None;
    }
    let hash_started = crate::perf::start();
    let hashes = hash_paths(state_root, source_root, &paths);
    if let Some(hash_started) = hash_started {
        // File lengths are observed with metadata only; the content reads happen
        // in Git's hash-object subprocesses, not in the profiler.
        let requested_bytes = paths
            .iter()
            .filter_map(|path| std::fs::metadata(source_root.join(path)).ok())
            .map(|metadata| metadata.len())
            .sum::<u64>();
        crate::perf::emit(
            "scan.source_index_hash",
            hash_started.elapsed(),
            &[
                ("requested_files", paths.len() as u64),
                ("requested_bytes", requested_bytes),
                ("hash_failed", u64::from(hashes.is_none())),
            ],
        );
    }
    let hashes = hashes?;
    if hashes.len() != paths.len() {
        return None;
    }

    Some(
        paths
            .into_iter()
            .zip(hashes)
            .filter_map(|(path, hash)| {
                let indexed = index.get(&path)?;
                let destination = mirror_root.join(&path);
                let present = std::fs::symlink_metadata(destination)
                    .is_ok_and(|metadata| metadata.file_type().is_file());
                (present && indexed == &hash).then_some(path)
            })
            .collect(),
    )
}

fn mirror_clean(state_root: &Path) -> Option<bool> {
    let output = Command::new("git")
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            "source",
        ])
        .current_dir(state_root)
        .output()
        .ok()?;
    output.status.success().then(|| output.stdout.is_empty())
}

fn source_index(state_root: &Path) -> Option<BTreeMap<PathBuf, String>> {
    let output = Command::new("git")
        .args(["ls-files", "-s", "-z", "--", "source"])
        .current_dir(state_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let mut result = BTreeMap::new();
    for record in output.stdout.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let tab = record.iter().position(|byte| *byte == b'\t')?;
        let metadata = String::from_utf8_lossy(&record[..tab]);
        let mut fields = metadata.split_whitespace();
        let _mode = fields.next()?;
        let hash = fields.next()?.to_owned();
        if fields.next()? != "0" {
            continue;
        }
        let path = String::from_utf8_lossy(&record[tab + 1..]);
        let relative = path.strip_prefix("source/")?;
        result.insert(PathBuf::from(relative), hash);
    }
    Some(result)
}

fn hash_paths(state_root: &Path, source_root: &Path, paths: &[PathBuf]) -> Option<Vec<String>> {
    if paths.is_empty() {
        return Some(Vec::new());
    }
    let workers = paths
        .len()
        .min(
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
        )
        .min(MAX_HASH_WORKERS)
        .max(1);
    let chunk_size = paths.len().div_ceil(workers);

    thread::scope(|scope| {
        let handles = paths
            .chunks(chunk_size)
            .map(|chunk| scope.spawn(move || hash_chunk(state_root, source_root, chunk)))
            .collect::<Vec<_>>();
        let mut result = Vec::with_capacity(paths.len());
        for handle in handles {
            result.extend(handle.join().ok()??);
        }
        Some(result)
    })
}

fn hash_chunk(state_root: &Path, source_root: &Path, paths: &[PathBuf]) -> Option<Vec<String>> {
    let git_dir = state_root.join(".git");
    let mut child = Command::new("git")
        .arg(format!("--git-dir={}", git_dir.display()))
        .args(["hash-object", "--stdin-paths", "--no-filters"])
        .current_dir(source_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    {
        let stdin = child.stdin.as_mut()?;
        for path in paths {
            let value = path.to_string_lossy().replace('\\', "/");
            writeln!(stdin, "{value}").ok()?;
        }
    }

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let hashes = String::from_utf8(output.stdout).ok()?;
    Some(
        hashes
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

#[cfg(test)]
#[path = "mirror_index_tests.rs"]
mod tests;
