use std::collections::BTreeMap;
use std::thread;

use super::analysis::RecordGroups;
use super::{FileEntry, LanguageEntry, StorageError, Store};

const MAX_FILE_WRITE_WORKERS: usize = 16;

pub(crate) fn write_full_file_objects(
    store: &Store,
    entry: &LanguageEntry,
    sources: BTreeMap<&str, &[u8]>,
    groups: &RecordGroups<'_>,
) -> Result<Vec<FileEntry>, StorageError> {
    let job_count = sources.len();
    if job_count == 0 {
        return Ok(Vec::new());
    }

    let workers = file_write_worker_count(
        job_count,
        thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
    );
    let jobs = sources
        .into_iter()
        .enumerate()
        .map(|(index, (path, source))| {
            let records = groups
                .owned
                .get(path)
                .map(Vec::as_slice)
                .unwrap_or_default();
            (index, path, source, records)
        })
        .collect::<Vec<_>>();

    if workers < 2 {
        return jobs
            .into_iter()
            .map(|(_, path, source, records)| {
                store.write_language_file_object(entry, path, source, records)
            })
            .collect();
    }

    let mut buckets = (0..workers).map(|_| Vec::new()).collect::<Vec<_>>();
    for job in jobs {
        buckets[job.0 % workers].push(job);
    }

    thread::scope(|scope| {
        let handles = buckets
            .into_iter()
            .map(|bucket| {
                scope.spawn(move || {
                    bucket
                        .into_iter()
                        .map(|(index, path, source, records)| {
                            let result =
                                store.write_language_file_object(entry, path, source, records);
                            (index, result)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();

        let mut results = (0..job_count)
            .map(|_| None)
            .collect::<Vec<Option<Result<FileEntry, StorageError>>>>();
        for handle in handles {
            let completed = handle.join().map_err(|_| {
                StorageError::Operation("file materialization worker panicked".into())
            })?;
            for (index, result) in completed {
                results[index] = Some(result);
            }
        }

        results
            .into_iter()
            .map(|result| {
                result.ok_or_else(|| {
                    StorageError::Operation("file materialization result missing".into())
                })?
            })
            .collect()
    })
}

fn file_write_worker_count(job_count: usize, available: usize) -> usize {
    job_count.min(available.max(1)).min(MAX_FILE_WRITE_WORKERS)
}

#[cfg(test)]
mod tests {
    use super::file_write_worker_count;

    #[test]
    fn file_write_workers_are_bounded_by_jobs_capacity_and_legacy_cap() {
        assert_eq!(file_write_worker_count(0, 32), 0);
        assert_eq!(file_write_worker_count(1, 32), 1);
        assert_eq!(file_write_worker_count(8, 4), 4);
        assert_eq!(file_write_worker_count(64, 64), 16);
        assert_eq!(file_write_worker_count(8, 0), 1);
    }
}
