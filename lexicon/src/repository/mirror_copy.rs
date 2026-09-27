use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use super::RepositoryError;

const MAX_COPY_WORKERS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CopyOutcome {
    Equal,
    Copied,
}

pub(crate) fn copy_all(
    root: &Path,
    desired: &BTreeMap<PathBuf, PathBuf>,
    indexed: Option<&BTreeSet<PathBuf>>,
) -> Result<(usize, usize), RepositoryError> {
    let jobs = desired
        .iter()
        .filter(|(relative, _)| indexed.is_none_or(|set| !set.contains(*relative)))
        .collect::<Vec<_>>();
    if jobs.is_empty() {
        return Ok((0, 0));
    }

    let workers = jobs
        .len()
        .min(
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
        )
        .min(MAX_COPY_WORKERS)
        .max(1);
    if workers == 1 {
        return summarize(
            jobs.into_iter()
                .map(|(relative, source)| copy_one(root, relative, source)),
        );
    }

    let mut buckets = (0..workers).map(|_| Vec::new()).collect::<Vec<_>>();
    for (index, job) in jobs.into_iter().enumerate() {
        buckets[index % workers].push(job);
    }

    thread::scope(|scope| {
        let handles = buckets
            .into_iter()
            .map(|bucket| {
                scope.spawn(move || {
                    summarize(
                        bucket
                            .into_iter()
                            .map(|(relative, source)| copy_one(root, relative, source)),
                    )
                })
            })
            .collect::<Vec<_>>();

        let mut copied = 0;
        let mut equal = 0;
        for handle in handles {
            let (worker_copied, worker_equal) = handle
                .join()
                .map_err(|_| RepositoryError::new("source mirror copy worker panicked"))??;
            copied += worker_copied;
            equal += worker_equal;
        }
        Ok((copied, equal))
    })
}

pub(crate) fn copy_one(
    root: &Path,
    relative: &Path,
    source: &Path,
) -> Result<CopyOutcome, RepositoryError> {
    let data = fs::read(source)?;
    let destination = root.join(relative);
    if fs::read(&destination).is_ok_and(|existing| existing == data) {
        return Ok(CopyOutcome::Equal);
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = PathBuf::from(format!("{}.lexicon-tmp", destination.display()));
    fs::write(&temporary, data)?;
    if let Err(first) = fs::rename(&temporary, &destination) {
        let _ = fs::remove_file(&destination);
        fs::rename(&temporary, &destination).map_err(|retry| {
            RepositoryError::new(format!(
                "replace mirror file {}: {retry}; initial error: {first}",
                destination.display()
            ))
        })?;
    }
    Ok(CopyOutcome::Copied)
}

fn summarize(
    results: impl Iterator<Item = Result<CopyOutcome, RepositoryError>>,
) -> Result<(usize, usize), RepositoryError> {
    let mut copied = 0;
    let mut equal = 0;
    for result in results {
        match result? {
            CopyOutcome::Copied => copied += 1,
            CopyOutcome::Equal => equal += 1,
        }
    }
    Ok((copied, equal))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn parallel_copy_preserves_bytes_and_reports_equal_repeats() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("lexicon-copy-{nonce}"));
        let source_root = root.join("source");
        let mirror_root = root.join("mirror");
        fs::create_dir_all(&source_root).expect("source root");

        let mut desired = BTreeMap::new();
        for index in 0..32 {
            let relative = PathBuf::from(format!("pkg/{index}.py"));
            let source = source_root.join(&relative);
            fs::create_dir_all(source.parent().expect("parent")).expect("parent dir");
            fs::write(&source, format!("value = {index}\n")).expect("source file");
            desired.insert(relative, source);
        }

        assert_eq!(
            copy_all(&mirror_root, &desired, None).expect("first copy"),
            (32, 0)
        );
        assert_eq!(
            copy_all(&mirror_root, &desired, None).expect("second copy"),
            (0, 32)
        );
        for (relative, source) in &desired {
            assert_eq!(
                fs::read(source).expect("source bytes"),
                fs::read(mirror_root.join(relative)).expect("mirror bytes")
            );
        }
        let _ = fs::remove_dir_all(root);
    }
}
