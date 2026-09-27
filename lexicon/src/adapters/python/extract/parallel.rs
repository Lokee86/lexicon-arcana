use std::ops::Range;
use std::sync::Arc;
use std::thread;

use crate::AdapterError;

use super::super::discovery::load;
use super::super::facts::Facts;
use super::super::model::{Repository, SourceInput};
use super::super::semantic;
use super::extract_file;
use super::lifetime::{ExtractionMetrics, LifetimeTracker};

pub fn extract_repository(
    repository: &Repository,
    facts: &mut Facts,
    workers: usize,
    shards: usize,
    merge_fan_in: usize,
) -> Result<ExtractionMetrics, AdapterError> {
    if repository.files.is_empty() {
        return Ok(ExtractionMetrics::default());
    }

    let ranges = partition_ranges(&repository.files, shards.max(1));
    let worker_count = workers.max(1).min(ranges.len());
    let tracker = Arc::new(LifetimeTracker::default());

    let fragments = if worker_count == 1 {
        ranges
            .iter()
            .enumerate()
            .map(|(index, range)| extract_range(repository, range.clone(), index, tracker.as_ref()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        extract_parallel(repository, &ranges, worker_count, Arc::clone(&tracker))?
    };

    let merged = reduce_fragments(fragments, merge_fan_in.max(2));
    facts.merge_from(merged);
    Ok(tracker.metrics(worker_count, ranges.len()))
}

fn extract_parallel(
    repository: &Repository,
    ranges: &[Range<usize>],
    workers: usize,
    tracker: Arc<LifetimeTracker>,
) -> Result<Vec<(usize, Facts)>, AdapterError> {
    let mut buckets = (0..workers).map(|_| Vec::new()).collect::<Vec<_>>();
    for (index, range) in ranges.iter().cloned().enumerate() {
        buckets[index % workers].push((index, range));
    }

    thread::scope(|scope| {
        let handles = buckets
            .into_iter()
            .map(|bucket| {
                let tracker = Arc::clone(&tracker);
                scope.spawn(move || {
                    bucket
                        .into_iter()
                        .map(|(index, range)| {
                            extract_range(repository, range, index, tracker.as_ref())
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
            })
            .collect::<Vec<_>>();

        let mut fragments = Vec::with_capacity(ranges.len());
        for handle in handles {
            let values = handle
                .join()
                .map_err(|_| AdapterError::new("Python extraction worker panicked"))??;
            fragments.extend(values);
        }
        fragments.sort_by_key(|(index, _)| *index);
        Ok(fragments)
    })
}

fn extract_range(
    repository: &Repository,
    range: Range<usize>,
    index: usize,
    tracker: &LifetimeTracker,
) -> Result<(usize, Facts), AdapterError> {
    let mut facts = Facts::new(repository.name.clone());
    for input in &repository.files[range] {
        let file = load(input)?;
        tracker.enter(&file);
        let result = extract_file(&file, &mut facts);
        if result.is_ok() {
            semantic::emit_file_facts(&file, &mut facts);
        }
        tracker.exit(&file);
        result?;
    }
    Ok((index, facts))
}

fn partition_ranges(files: &[SourceInput], count: usize) -> Vec<Range<usize>> {
    if files.is_empty() {
        return Vec::new();
    }
    let count = count.max(1).min(files.len());
    if count == 1 {
        return vec![0..files.len()];
    }

    let total = files.iter().map(|file| file.size.max(1)).sum::<u64>();
    let target = total.div_ceil(count as u64).max(1);
    let mut ranges = Vec::with_capacity(count);
    let mut start = 0;
    let mut weight = 0_u64;

    for (index, file) in files.iter().enumerate() {
        let remaining_files = files.len() - index;
        let remaining_ranges = count - ranges.len();
        let file_weight = file.size.max(1);
        if index > start
            && ranges.len() < count - 1
            && weight + file_weight > target
            && remaining_files >= remaining_ranges
        {
            ranges.push(start..index);
            start = index;
            weight = 0;
        }
        weight += file_weight;
    }
    ranges.push(start..files.len());
    ranges
}

fn reduce_fragments(mut current: Vec<(usize, Facts)>, fan_in: usize) -> Facts {
    current.sort_by_key(|(index, _)| *index);
    while current.len() > 1 {
        let mut next = Vec::with_capacity(current.len().div_ceil(fan_in));
        let mut iterator = current.into_iter();
        while let Some((index, mut facts)) = iterator.next() {
            for _ in 1..fan_in {
                let Some((_, source)) = iterator.next() else {
                    break;
                };
                facts.merge_from(source);
            }
            next.push((index, facts));
        }
        current = next;
    }
    current
        .pop()
        .map(|(_, facts)| facts)
        .unwrap_or_else(|| Facts::new(String::new()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::partition_ranges;
    use crate::adapters::python::model::SourceInput;

    fn input(size: u64) -> SourceInput {
        SourceInput {
            path: PathBuf::new(),
            relative: String::new(),
            module: String::new(),
            size,
        }
    }

    #[test]
    fn partitions_cover_inputs_in_stable_contiguous_ranges() {
        let files = vec![input(8), input(1), input(7), input(2), input(6)];
        let ranges = partition_ranges(&files, 3);
        assert_eq!(ranges.first().map(|range| range.start), Some(0));
        assert_eq!(ranges.last().map(|range| range.end), Some(files.len()));
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }
}
